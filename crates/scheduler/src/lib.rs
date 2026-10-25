//! Cron and wake-time scheduling (roadmap Phase 6; `docs/features/
//! scheduled-tasks.md`, `REQ-SCHED-02/03`, `TEST-SCHED-002/003`).
//!
//! This crate is pure computation: it owns the schedule grammar, next-run
//! arithmetic, and the arm/catch-up state machine. It has no I/O and no
//! session knowledge — the store owns the rows and the bridge owns execution,
//! so a scheduled run reuses exactly the interactive send path.
//!
//! Everything here is **rebuild-defined**: the shipped editor was never
//! exercised and the shipped schedule grammar was not captured (`docs/14`
//! §9). The documented facts honored are behavioral: a missed run is recorded
//! as a skip, never silently backfilled (`TEST-SCHED-003`); a run creates a
//! normal session (the bridge does that, this crate only says when); and
//! keep-awake is out of scope here entirely (it cannot override lid close or
//! power-off, `REQ-SCHED-06`, and ships disabled — `TEST-SCHED-005`).
//!
//! ## The grammar
//!
//! A task stores a 5-field cron string (`minute hour day-of-month month
//! day-of-week`). The supported subset is explicit and refused otherwise:
//! `*`, exact values, comma lists, and `*/step` (minute/hour only). Day-of-
//! month and month must be `*` — named months and month-day combinations were
//! never observed and an unexercised surface is worse than an honest refusal.
//!
//! ## DST and missing times
//!
//! Next-run arithmetic is resolved through a `chrono::TimeZone`, so the
//! device zone decides. The two recorded rules:
//!
//! - A wall-clock time that falls inside a spring-forward gap **does not
//!   occur**; the next run is the next matching wall-clock time (a daily
//!   01:30 task skips the spring-forward morning entirely).
//! - A fall-back ambiguity resolves to the **earlier** occurrence.
//!
//! ## The arm/catch-up state machine
//!
//! A task is *armed* for one occurrence at a time (`arm`). A `decide` answer
//! tells the caller to run or skip: an occurrence whose moment passed by more
//! than [`GRACE_MILLIS`] is a skip ("the computer was asleep, offline, or the
//! app was closed"), never a late execution and never a silent drop. After a
//! run or a skip the caller rearms, which strictly advances past the handled
//! occurrence.

use chrono::{DateTime, Datelike, TimeZone, Timelike};
use std::collections::BTreeSet;

/// How late an armed occurrence may fire before it becomes a skip. The app
/// tick is per-minute; one minute of scheduler noise must not record a skip.
pub const GRACE_MILLIS: i64 = 60_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum CronError {
    #[error("the schedule must have exactly 5 fields")]
    FieldCount,
    #[error("schedule field {0} is not a number, a list, a step, or '*'")]
    Malformed(usize),
    #[error("schedule field {0} is out of range")]
    OutOfRange(usize),
    #[error("day-of-month and month must be '*' in this build's schedule subset")]
    UnsupportedField(usize),
    #[error("the schedule never fires (an empty list matches nothing)")]
    Never,
}

/// One parsed cron schedule: the minute/hour sets and, when restricted, the
/// allowed weekdays (0 = Sunday … 6 = Saturday, cron convention).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    minutes: BTreeSet<u32>,
    hours: BTreeSet<u32>,
    days_of_week: Option<BTreeSet<u32>>,
}

fn parse_field(
    field: &str,
    index: usize,
    min: u32,
    max: u32,
    allow_step: bool,
) -> Result<BTreeSet<u32>, CronError> {
    let mut out = BTreeSet::new();
    for part in field.split(',') {
        if let Some(step_text) = part.strip_prefix("*/") {
            if !allow_step {
                return Err(CronError::Malformed(index));
            }
            let step: u32 = step_text.parse().map_err(|_| CronError::Malformed(index))?;
            if step == 0 {
                return Err(CronError::Malformed(index));
            }
            let mut value = min;
            while value <= max {
                out.insert(value);
                value += step;
            }
        } else if part == "*" {
            let mut value = min;
            while value <= max {
                out.insert(value);
                value += 1;
            }
        } else if let Some((from_text, to_text)) = part.split_once('-') {
            // A range `a-b` is inclusive; reversed ranges are refused rather
            // than wrapped (an honest refusal over a surprise wrap).
            let from: u32 = from_text.parse().map_err(|_| CronError::Malformed(index))?;
            let to: u32 = to_text.parse().map_err(|_| CronError::Malformed(index))?;
            if from < min || to > max || from > to {
                return Err(CronError::OutOfRange(index));
            }
            for value in from..=to {
                out.insert(value);
            }
        } else {
            let value: u32 = part.parse().map_err(|_| CronError::Malformed(index))?;
            if value < min || value > max {
                return Err(CronError::OutOfRange(index));
            }
            out.insert(value);
        }
    }
    if out.is_empty() {
        return Err(CronError::Never);
    }
    Ok(out)
}

impl CronSchedule {
    /// Parse the 5-field subset documented in the module header.
    pub fn parse(cron: &str) -> Result<CronSchedule, CronError> {
        let fields: Vec<&str> = cron.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(CronError::FieldCount);
        }
        let minutes = parse_field(fields[0], 0, 0, 59, true)?;
        let hours = parse_field(fields[1], 1, 0, 23, true)?;
        if fields[2] != "*" {
            return Err(CronError::UnsupportedField(2));
        }
        if fields[3] != "*" {
            return Err(CronError::UnsupportedField(3));
        }
        let days_of_week = if fields[4] == "*" {
            None
        } else {
            // Cron's 7 is an alias for Sunday; normalize it to 0.
            let mut days = parse_field(fields[4], 4, 0, 7, false)?;
            if days.remove(&7) {
                days.insert(0);
            }
            Some(days)
        };
        Ok(CronSchedule {
            minutes,
            hours,
            days_of_week,
        })
    }

    /// A short human summary of the schedule (rebuild-defined copy).
    pub fn describe(&self) -> String {
        let mut times = String::new();
        for (index, hour) in self.hours.iter().enumerate() {
            for minute in &self.minutes {
                if !times.is_empty() {
                    times.push_str(", ");
                }
                times.push_str(&format!("{hour:02}:{minute:02}"));
            }
            let _ = index;
        }
        match &self.days_of_week {
            None => format!("every day at {times}"),
            Some(days) if days.len() == 7 => format!("every day at {times}"),
            Some(days) => {
                const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
                let names = days
                    .iter()
                    .map(|d| NAMES[*d as usize])
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("on {names} at {times}")
            }
        }
    }

    /// The next matching instant strictly after `after`, resolved in `tz`.
    ///
    /// Iterates candidate wall-clock minutes (bounded at 8 days) and resolves
    /// each through the zone, so a spring-forward gap skips the missing
    /// minute (documented rule) and a fall-back ambiguity takes the earlier
    /// occurrence. `None` means no matching real minute exists within the
    /// window; the caller treats that as an unschedulable task rather than
    /// inventing a time.
    pub fn next_after<Tz: TimeZone>(&self, tz: &Tz, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        // Start at the next whole minute strictly after `after`.
        let mut candidate = after.clone();
        candidate = candidate.with_nanosecond(0).map(|c| c + chrono::Duration::minutes(1))?;
        for _ in 0..(8 * 24 * 60) {
            if self.matches_wall(candidate.clone()) {
                match tz.from_local_datetime(&candidate.naive_local()) {
                    // A fall-back ambiguity takes the earlier occurrence.
                    chrono::LocalResult::Single(resolved) => return Some(resolved),
                    chrono::LocalResult::Ambiguous(earliest, _) => return Some(earliest),
                    // A spring-forward gap: this wall-clock minute does not
                    // exist; keep scanning (documented rule).
                    chrono::LocalResult::None => {}
                }
            }
            candidate += chrono::Duration::minutes(1);
        }
        None
    }

    fn matches_wall<Tz: TimeZone>(&self, candidate: DateTime<Tz>) -> bool {
        if !self.minutes.contains(&candidate.minute()) || !self.hours.contains(&candidate.hour()) {
            return false;
        }
        match &self.days_of_week {
            None => true,
            Some(days) => days.contains(&candidate.weekday().num_days_from_sunday()),
        }
    }
}

/// The arm/catch-up decision for one armed occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occurrence {
    /// Fire now: the armed moment is within the grace window.
    Run,
    /// Record a skip with the recorded reason; never execute late.
    Skip,
}

/// Decide what to do with an armed occurrence at `now`.
pub fn decide(armed_at: i64, now: i64) -> Occurrence {
    // Negative (the clock sits behind the armed moment, e.g. after a skew
    // correction) is not "late": the occurrence fires on the next tick.
    let missed_by = now - armed_at;
    if missed_by <= GRACE_MILLIS {
        Occurrence::Run
    } else {
        Occurrence::Skip
    }
}

/// The skip reason recorded for a missed occurrence (`TEST-SCHED-003`).
pub fn skip_reason() -> &'static str {
    "the computer was asleep, offline, or the app was closed at the scheduled time"
}

#[cfg(test)]
mod tests {
    use super::{decide, skip_reason, CronError, CronSchedule, Occurrence, GRACE_MILLIS};
    use chrono::{FixedOffset, LocalResult, NaiveDateTime, TimeZone, Utc};

    fn zone(hours: i32) -> FixedOffset {
        FixedOffset::east_opt(hours * 3600).expect("valid offset")
    }

    #[test]
    fn parses_the_documented_subset_and_refuses_the_rest() {
        let daily = CronSchedule::parse("30 9 * * *").expect("daily");
        assert_eq!(daily.describe(), "every day at 09:30");

        let weekly = CronSchedule::parse("0 8 * * 1,3,5").expect("weekly");
        assert_eq!(weekly.describe(), "on Mon, Wed, Fri at 08:00");

        // A step expands to the expected count without spelling 96 items out.
        let stepped = CronSchedule::parse("*/15 * * * *").expect("stepped");
        let described = stepped.describe();
        assert!(described.starts_with("every day at 00:00, 00:15"));
        assert!(described.ends_with("23:30, 23:45"));
        assert_eq!(described.matches(", ").count(), 95, "96 times = 95 separators");

        assert_eq!(CronSchedule::parse("30 9 * *"), Err(CronError::FieldCount));
        assert_eq!(CronSchedule::parse("30 9 1 * *"), Err(CronError::UnsupportedField(2)));
        assert_eq!(CronSchedule::parse("30 9 * 2 *"), Err(CronError::UnsupportedField(3)));
        assert_eq!(CronSchedule::parse("60 9 * * *"), Err(CronError::OutOfRange(0)));
        assert_eq!(CronSchedule::parse("ab 9 * * *"), Err(CronError::Malformed(0)));
        assert_eq!(CronSchedule::parse("*/0 * * * *"), Err(CronError::Malformed(0)));
        // Weekday ranges are the templates' "weekdays" shape.
        let weekdays = CronSchedule::parse("0 8 * * 1-5").expect("range");
        assert_eq!(weekdays.describe(), "on Mon, Tue, Wed, Thu, Fri at 08:00");
        assert_eq!(CronSchedule::parse("0 8 * * 5-1"), Err(CronError::OutOfRange(4)));
        // Cron's 7 is Sunday's alias.
        let sunday_alias = CronSchedule::parse("0 12 * * 7").expect("alias");
        assert_eq!(sunday_alias.describe(), "on Sun at 12:00");
    }

    #[test]
    fn next_run_respects_the_task_zone_not_the_process_zone() {
        let daily = CronSchedule::parse("30 9 * * *").expect("daily");
        // 09:30 in UTC+05:00 is 04:30 UTC. The schedule must land on the
        // zone's wall clock, not the process's.
        let tz = zone(5);
        let after = tz
            .with_ymd_and_hms(2026, 10, 26, 10, 0, 0)
            .single()
            .expect("valid local time");
        let next = daily.next_after(&tz, &after).expect("next run");
        assert_eq!(next.format("%Y-%m-%d %H:%M").to_string(), "2026-10-27 09:30");
        assert_eq!(next.offset(), &tz);
        // The same wall clock in another zone is a different UTC instant.
        let utc_a = next.with_timezone(&Utc).format("%Y-%m-%dT%H:%M").to_string();
        assert_eq!(utc_a, "2026-10-27T04:30");
    }

    #[test]
    fn weekly_days_advance_to_the_next_allowed_day() {
        let weekly = CronSchedule::parse("0 8 * * 1,3,5").expect("weekly");
        let tz = zone(0);
        // 2026-10-26 is a Monday; 08:00 already passed at 09:00.
        let after = tz
            .with_ymd_and_hms(2026, 10, 26, 9, 0, 0)
            .single()
            .expect("valid");
        let next = weekly.next_after(&tz, &after).expect("next");
        assert_eq!(
            next.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-28 08:00",
            "Wednesday"
        );
    }

    #[test]
    fn a_gap_skipped_morning_moves_to_the_next_day_not_the_next_minute() {
        // A hand-rolled zone with a spring-forward gap: 2026-03-08 02:00-02:59
        // does not exist (standard UTC-8 -> daylight UTC-7 at 02:00 local).
        #[derive(Clone, Copy, Debug)]
        struct GapZone;
        const STANDARD: i32 = -8 * 3600;
        const DAYLIGHT: i32 = -7 * 3600;


        impl TimeZone for GapZone {
            type Offset = FixedOffset;
            fn from_offset(offset: &FixedOffset) -> GapZone {
                let _ = offset;
                GapZone
            }
            fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
                let start = NaiveDateTime::parse_from_str("2026-03-08 02:00", "%Y-%m-%d %H:%M").unwrap();
                let end = NaiveDateTime::parse_from_str("2026-03-08 03:00", "%Y-%m-%d %H:%M").unwrap();
                if *local >= start && *local < end {
                    LocalResult::None
                } else if *local < start {
                    LocalResult::Single(FixedOffset::east_opt(STANDARD).unwrap())
                } else {
                    LocalResult::Single(FixedOffset::east_opt(DAYLIGHT).unwrap())
                }
            }
            fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
                // 02:00 local standard (UTC-8) is 10:00 UTC.
                let cutoff = NaiveDateTime::parse_from_str("2026-03-08 10:00", "%Y-%m-%d %H:%M").unwrap();
                if *utc < cutoff {
                    FixedOffset::east_opt(STANDARD).unwrap()
                } else {
                    FixedOffset::east_opt(DAYLIGHT).unwrap()
                }
            }
            fn offset_from_local_date(
                &self,
                date: &chrono::NaiveDate,
            ) -> LocalResult<FixedOffset> {
                self.offset_from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
            }
            fn offset_from_utc_date(&self, date: &chrono::NaiveDate) -> FixedOffset {
                self.offset_from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
            }
        }

        let daily = CronSchedule::parse("30 2 * * *").expect("daily 02:30");
        let after = GapZone.from_utc_datetime(
            &NaiveDateTime::parse_from_str("2026-03-07 11:00", "%Y-%m-%d %H:%M").unwrap(),
        );
        let next = daily.next_after(&GapZone, &after).expect("next");
        // 2026-03-08 02:30 falls inside the gap, so the next run is the next
        // day's 02:30 (now at daylight offset) - never a shifted minute on
        // the same morning (documented rule; TEST-SCHED-002).
        assert_eq!(next.format("%Y-%m-%d %H:%M").to_string(), "2026-03-09 02:30");
    }

    #[test]
    fn the_arm_decision_runs_in_grace_and_skips_late() {
        assert_eq!(decide(1_000, 1_000), Occurrence::Run);
        assert_eq!(decide(1_000, 1_000 + GRACE_MILLIS), Occurrence::Run);
        assert_eq!(decide(1_000, 1_000 + GRACE_MILLIS + 1), Occurrence::Skip);
        // A clock behind the armed moment never skips backwards.
        assert_eq!(decide(2_000, 1_000), Occurrence::Run);
        assert_eq!(
            skip_reason(),
            "the computer was asleep, offline, or the app was closed at the scheduled time"
        );
    }
}
