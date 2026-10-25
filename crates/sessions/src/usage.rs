//! Usage aggregation over local-day windows (`TEST-SESSION-008`,
//! `TEST-DATA-007`).
//!
//! The ledger stores raw events with millisecond timestamps. The usage page's
//! 7/30/90-day views are *local-day* views: an event at 23:30 local belongs to
//! that day even when its UTC date has already rolled over. Aggregation
//! therefore takes the device's UTC offset explicitly (`FixedOffset`), which
//! keeps the math testable across zones instead of reading the process
//! timezone implicitly. The offset is the device's *current* offset applied to
//! the whole window; a DST transition inside the window is not re-bucketed
//! (the shipped app's exact behavior across a transition was not observed).
//!
//! Cached tokens are summed separately and are never folded into input/output
//! (`docs/11` §6: the Usage page states they are excluded from token limits).
//! Absent channels are stored as `0` by the agent's ledger write; the turn's
//! `usage_json` remains the exact provider shape.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, FixedOffset, TimeZone, Timelike};
use myharness_store::{CodeTurnActivity, StoreError, UsageEventRow};
use serde::{Deserialize, Serialize};

use crate::{AuditKeyStore, SessionError, SessionService};

/// The largest window the usage view may ask for. 7/30/90 are the documented
/// views; a year is the outer bound that still keeps the day buckets finite.
const MAX_WINDOW_DAYS: u32 = 366;

/// The all-time Code daily strip is capped at ten years of buckets. The flag is
/// reported rather than silently relabeling the strip as complete; all-time
/// totals are not capped.
const MAX_CODE_WINDOW_DAYS: u32 = 3660;

/// One window's token totals. `events` counts ledger rows (attempts);
/// `error_events` counts the attempts that ended in a provider error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub search_requests: i64,
    pub events: i64,
    pub error_events: i64,
}

/// One local day in the window. Days with no events are present with zero
/// totals so the chart does not silently collapse gaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    /// Local date, `YYYY-MM-DD`.
    pub date: String,
    pub totals: UsageTotals,
}

/// The usage page's data: a closed local-day window and its per-day buckets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub days: u32,
    /// The device offset the window was computed in, minutes east of UTC.
    pub timezone_offset_minutes: i32,
    /// Inclusive window start, RFC 3339 in the device offset.
    pub from: String,
    /// Inclusive window end, RFC 3339 in the device offset.
    pub to: String,
    pub totals: UsageTotals,
    pub daily: Vec<DailyUsage>,
}

/// The first millisecond of the oldest local day in the window.
pub(crate) fn window_start_ms(
    days: u32,
    tz: FixedOffset,
    now_ms: i64,
) -> Result<i64, SessionError> {
    if !(1..=MAX_WINDOW_DAYS).contains(&days) {
        return Err(StoreError::Validation(
            "the usage window must be between 1 and 366 days",
        )
        .into());
    }
    local_window_start_ms(days, tz, now_ms)
}

/// The first millisecond of the oldest local day, with only the finite-series
/// bound (not the usage page's one-year bound). The Code all-time view uses
/// this and reports its own cap.
fn local_window_start_ms(
    days: u32,
    tz: FixedOffset,
    now_ms: i64,
) -> Result<i64, SessionError> {
    if days == 0 {
        return Err(StoreError::Validation("the usage window needs at least one day").into());
    }
    let now = tz
        .timestamp_millis_opt(now_ms)
        .single()
        .ok_or(StoreError::Validation("the usage window time is out of range"))?;
    let start = now.date_naive() - Duration::days(i64::from(days) - 1);
    let midnight = start
        .and_hms_opt(0, 0, 0)
        .expect("midnight is always a valid local time");
    let start = match tz.from_local_datetime(&midnight) {
        chrono::LocalResult::Single(start) => start,
        // A repeated midnight (fall-back) belongs to the first occurrence.
        chrono::LocalResult::Ambiguous(first, _) => first,
        // Some zones jump over midnight (spring-forward). The first instant of
        // that local day is the transition, so probe an hour into the day.
        chrono::LocalResult::None => tz
            .from_local_datetime(&(midnight + Duration::hours(1)))
            .earliest()
            .ok_or(StoreError::Validation("the usage window start is out of range"))?,
    };
    Ok(start.timestamp_millis())
}

/// Fold ledger rows into local-day buckets. Pure: the caller supplies `now`
/// and the offset, so timezone-boundary behavior is directly testable.
pub fn summarize_usage(
    events: &[UsageEventRow],
    days: u32,
    tz: FixedOffset,
    now_ms: i64,
) -> Result<UsageSummary, SessionError> {
    let from_ms = window_start_ms(days, tz, now_ms)?;
    let now_local = tz
        .timestamp_millis_opt(now_ms)
        .single()
        .ok_or(StoreError::Validation("the usage window time is out of range"))?;
    let start_date = now_local.date_naive() - Duration::days(i64::from(days) - 1);

    let mut daily: Vec<DailyUsage> = (0..days)
        .map(|offset| DailyUsage {
            date: (start_date + Duration::days(i64::from(offset)))
                .format("%Y-%m-%d")
                .to_string(),
            totals: UsageTotals::default(),
        })
        .collect();
    let mut totals = UsageTotals::default();

    for event in events {
        // The store query already applies these bounds; keeping them here makes
        // the pure function safe on its own (and documents the closed window).
        if event.timestamp < from_ms || event.timestamp > now_ms {
            continue;
        }
        let Some(local) = tz.timestamp_millis_opt(event.timestamp).single() else {
            continue;
        };
        let index = (local.date_naive() - start_date).num_days();
        if !(0..i64::from(days)).contains(&index) {
            continue;
        }
        add(&mut daily[index as usize].totals, event);
        add(&mut totals, event);
    }

    let from_local = tz
        .timestamp_millis_opt(from_ms)
        .single()
        .expect("the window start came from this offset");

    Ok(UsageSummary {
        days,
        timezone_offset_minutes: tz.local_minus_utc() / 60,
        from: from_local.to_rfc3339(),
        to: now_local.to_rfc3339(),
        totals,
        daily,
    })
}

fn add(totals: &mut UsageTotals, event: &UsageEventRow) {
    totals.input_tokens = totals.input_tokens.saturating_add(event.input_tokens);
    totals.output_tokens = totals.output_tokens.saturating_add(event.output_tokens);
    totals.cache_read_tokens = totals.cache_read_tokens.saturating_add(event.cache_read_tokens);
    totals.cache_write_tokens = totals
        .cache_write_tokens
        .saturating_add(event.cache_write_tokens);
    totals.search_requests = totals.search_requests.saturating_add(event.search_requests);
    totals.events = totals.events.saturating_add(1);
    if event.error.is_some() {
        totals.error_events = totals.error_events.saturating_add(1);
    }
}

/// The three dashboard ranges observed on the Code surface (`docs/features/code.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeRange {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "all")]
    All,
}

/// One model's share of the Code ledger. `share_percent` is the share of
/// input + output tokens, rounded only for display by the renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeModelUsage {
    pub model: String,
    pub totals: UsageTotals,
    pub sessions: i64,
    pub share_percent: f64,
}

/// The Code dashboard's data. It is derived only from Code sessions and their
/// turns/usage events — never from Cowork rows (`TEST-CODE-001`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeUsageSummary {
    pub range: CodeRange,
    pub days: u32,
    /// True when the all-time daily strip was capped before the earliest
    /// event. Totals, messages, sessions, active days, and model shares still
    /// cover the whole range.
    pub truncated: bool,
    pub timezone_offset_minutes: i32,
    pub from: String,
    pub to: String,
    pub totals: UsageTotals,
    pub daily: Vec<DailyUsage>,
    pub messages: i64,
    pub sessions: i64,
    pub active_days: i64,
    pub peak_hour: Option<u32>,
    pub favorite_model: Option<String>,
    pub models: Vec<CodeModelUsage>,
}

fn code_window_days(
    range: CodeRange,
    tz: FixedOffset,
    now_ms: i64,
    earliest_ms: Option<i64>,
) -> Result<(u32, bool), SessionError> {
    let fixed = match range {
        CodeRange::SevenDays => Some(7),
        CodeRange::ThirtyDays => Some(30),
        CodeRange::All => None,
    };
    if let Some(days) = fixed {
        return Ok((days, false));
    }

    let now_local = tz
        .timestamp_millis_opt(now_ms)
        .single()
        .ok_or(StoreError::Validation("the usage window time is out of range"))?;
    let today = now_local.date_naive();
    let start = earliest_ms
        .and_then(|ms| tz.timestamp_millis_opt(ms).single())
        .map(|local| local.date_naive())
        .unwrap_or(today);
    let raw_days = (today - start).num_days().saturating_add(1).max(1);
    if raw_days > i64::from(MAX_CODE_WINDOW_DAYS) {
        Ok((MAX_CODE_WINDOW_DAYS, true))
    } else {
        Ok((raw_days as u32, false))
    }
}

/// Fold Code-only ledger rows and turns into the dashboard summary. Pure: the
/// caller supplies the clock and offset, so boundary behavior is testable.
/// Turns are counted only when they are `user`/`assistant` rows inside the
/// summary window; for all-time that window starts at the earliest Code
/// activity (event or turn), even when the daily strip is capped.
pub fn summarize_code_usage(
    events: &[UsageEventRow],
    turns: &[CodeTurnActivity],
    range: CodeRange,
    tz: FixedOffset,
    now_ms: i64,
    earliest_ms: Option<i64>,
) -> Result<CodeUsageSummary, SessionError> {
    let (days, truncated) = code_window_days(range, tz, now_ms, earliest_ms)?;
    let series_from_ms = local_window_start_ms(days, tz, now_ms)?;
    // All-time totals are genuinely all-time. Only the daily strip is capped,
    // because a finite series cannot grow without bound; `truncated` says the
    // strip starts after the earliest activity, never that the totals omitted
    // it.
    let summary_from_ms = if range == CodeRange::All {
        earliest_ms.unwrap_or(now_ms).min(now_ms)
    } else {
        series_from_ms
    };
    summarize_code_window(
        events,
        turns,
        range,
        days,
        truncated,
        tz,
        now_ms,
        summary_from_ms,
        series_from_ms,
    )
}

#[allow(clippy::too_many_arguments)]
fn summarize_code_window(
    events: &[UsageEventRow],
    turns: &[CodeTurnActivity],
    range: CodeRange,
    days: u32,
    truncated: bool,
    tz: FixedOffset,
    now_ms: i64,
    summary_from_ms: i64,
    series_from_ms: i64,
) -> Result<CodeUsageSummary, SessionError> {
    let now_local = tz
        .timestamp_millis_opt(now_ms)
        .single()
        .ok_or(StoreError::Validation("the usage window time is out of range"))?;
    let series_start_date = now_local.date_naive() - Duration::days(i64::from(days) - 1);

    let mut daily: Vec<DailyUsage> = (0..days)
        .map(|offset| DailyUsage {
            date: (series_start_date + Duration::days(i64::from(offset)))
                .format("%Y-%m-%d")
                .to_string(),
            totals: UsageTotals::default(),
        })
        .collect();
    let mut totals = UsageTotals::default();
    let mut sessions = BTreeSet::new();
    let mut active_dates = BTreeSet::new();
    let mut model_events: BTreeMap<String, i64> = BTreeMap::new();
    let mut model_totals: BTreeMap<String, (UsageTotals, BTreeSet<String>)> = BTreeMap::new();
    let mut hour_events = [0i64; 24];

    for event in events {
        if event.timestamp < summary_from_ms || event.timestamp > now_ms {
            continue;
        }
        let Some(local) = tz.timestamp_millis_opt(event.timestamp).single() else {
            continue;
        };
        if event.timestamp >= series_from_ms {
            let index = (local.date_naive() - series_start_date).num_days();
            if (0..i64::from(days)).contains(&index) {
                add(&mut daily[index as usize].totals, event);
            }
        }
        add(&mut totals, event);
        sessions.insert(event.session_id.clone());
        active_dates.insert(local.date_naive());
        *model_events.entry(event.model.clone()).or_insert(0) += 1;
        let entry = model_totals
            .entry(event.model.clone())
            .or_insert_with(|| (UsageTotals::default(), BTreeSet::new()));
        add(&mut entry.0, event);
        entry.1.insert(event.session_id.clone());
        hour_events[local.hour() as usize] = hour_events[local.hour() as usize].saturating_add(1);
    }

    let messages = turns
        .iter()
        .filter(|turn| {
            turn.created_at >= summary_from_ms
                && turn.created_at <= now_ms
                && matches!(turn.role.as_str(), "user" | "assistant")
        })
        .count() as i64;

    let active_days = active_dates.len() as i64;

    let mut peak_hour = None;
    let mut peak_count = 0i64;
    for (hour, count) in hour_events.iter().enumerate() {
        if *count > peak_count {
            peak_count = *count;
            peak_hour = Some(hour as u32);
        }
    }

    let mut favorite_model: Option<String> = None;
    let mut favorite_count = 0i64;
    for (model, count) in &model_events {
        let wins = match &favorite_model {
            None => true,
            Some(current) => {
                *count > favorite_count
                    || (*count == favorite_count && model.as_str() < current.as_str())
            }
        };
        if wins {
            favorite_model = Some(model.clone());
            favorite_count = *count;
        }
    }

    let total_tokens = totals.input_tokens.saturating_add(totals.output_tokens);
    let mut models: Vec<CodeModelUsage> = model_totals
        .into_iter()
        .map(|(model, (totals, sessions))| {
            let tokens = totals.input_tokens.saturating_add(totals.output_tokens);
            let share_percent = if total_tokens > 0 {
                (tokens as f64 / total_tokens as f64) * 100.0
            } else {
                0.0
            };
            CodeModelUsage {
                model,
                totals,
                sessions: sessions.len() as i64,
                share_percent,
            }
        })
        .collect();
    models.sort_by(|a, b| {
        let a_tokens = a.totals.input_tokens.saturating_add(a.totals.output_tokens);
        let b_tokens = b.totals.input_tokens.saturating_add(b.totals.output_tokens);
        b_tokens.cmp(&a_tokens).then_with(|| a.model.cmp(&b.model))
    });

    let from_local = tz
        .timestamp_millis_opt(summary_from_ms)
        .single()
        .ok_or(StoreError::Validation("the summary start is out of range"))?;

    Ok(CodeUsageSummary {
        range,
        days,
        truncated,
        timezone_offset_minutes: tz.local_minus_utc() / 60,
        from: from_local.to_rfc3339(),
        to: now_local.to_rfc3339(),
        totals,
        daily,
        messages,
        sessions: sessions.len() as i64,
        active_days,
        peak_hour,
        favorite_model,
        models,
    })
}

impl<K: AuditKeyStore> SessionService<K> {
    /// The device-wide ledger (or one session's) summarized over a local-day
    /// window. `now_ms` is caller-supplied so the bridge and tests agree on the
    /// boundary instant.
    pub fn usage_summary(
        &self,
        session_id: Option<&str>,
        days: u32,
        tz: FixedOffset,
        now_ms: i64,
    ) -> Result<UsageSummary, SessionError> {
        let from_ms = window_start_ms(days, tz, now_ms)?;
        let events = self.store.list_usage_between(from_ms, now_ms, session_id)?;
        summarize_usage(&events, days, tz, now_ms)
    }

    /// Code-only usage for the dashboard. The event and turn queries join on
    /// `sessions.surface = 'code'`, so Cowork data cannot be counted here.
    pub fn code_usage_summary(
        &self,
        range: CodeRange,
        tz: FixedOffset,
        now_ms: i64,
    ) -> Result<CodeUsageSummary, SessionError> {
        let earliest = self.store.earliest_code_activity_ms()?;
        let (days, truncated) = code_window_days(range, tz, now_ms, earliest)?;
        let series_from_ms = local_window_start_ms(days, tz, now_ms)?;
        // All-time totals include every Code event, even when the daily strip
        // is capped; the finite 7/30-day windows use the same start for both.
        let summary_from_ms = if range == CodeRange::All {
            earliest.unwrap_or(now_ms).min(now_ms)
        } else {
            series_from_ms
        };
        let events = self
            .store
            .list_code_usage_between(summary_from_ms, now_ms)?;
        let turns = self
            .store
            .list_code_turns_between(summary_from_ms, now_ms)?;
        summarize_code_window(
            &events,
            &turns,
            range,
            days,
            truncated,
            tz,
            now_ms,
            summary_from_ms,
            series_from_ms,
        )
    }
}
