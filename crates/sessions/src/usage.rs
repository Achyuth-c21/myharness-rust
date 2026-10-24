//! Usage aggregation over local-day windows (`TEST-SESSION-008`,
//! `TEST-DATA-007`).
//!
//! The ledger stores raw events with millisecond timestamps. The usage page's
//! 7/30/90-day views are *local-day* views: an event at 23:30 local belongs to
//! that day even when its UTC date has already rolled over. Aggregation
//! therefore takes the device's UTC offset explicitly (`FixedOffset`), which
//! keeps the math testable across zones instead of reading the process
//! timezone implicitly.
//!
//! Cached tokens are summed separately and are never folded into input/output
//! (`docs/11` §6: the Usage page states they are excluded from token limits).
//! Absent channels are stored as `0` by the agent's ledger write; the turn's
//! `usage_json` remains the exact provider shape.

use chrono::{Duration, FixedOffset, TimeZone};
use myharness_store::{StoreError, UsageEventRow};
use serde::{Deserialize, Serialize};

use crate::{AuditKeyStore, SessionError, SessionService};

/// The largest window the usage view may ask for. 7/30/90 are the documented
/// views; a year is the outer bound that still keeps the day buckets finite.
const MAX_WINDOW_DAYS: u32 = 366;

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
    let now = tz
        .timestamp_millis_opt(now_ms)
        .single()
        .ok_or(StoreError::Validation("the usage window time is out of range"))?;
    let start = now.date_naive() - Duration::days(i64::from(days) - 1);
    let midnight = start
        .and_hms_opt(0, 0, 0)
        .expect("midnight is always a valid local time");
    let start = tz
        .from_local_datetime(&midnight)
        .single()
        .ok_or(StoreError::Validation("the usage window start is out of range"))?;
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
}
