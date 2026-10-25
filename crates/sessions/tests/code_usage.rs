//! Code-only dashboard aggregation (`TEST-CODE-001`).
//!
//! These tests pin the boundary that matters for the dashboard: Cowork rows
//! must never contribute, the 7/30-day windows are closed local-day windows,
//! and the all-time totals cover every real Code event while only the daily
//! strip is capped instead of inventing a fixed history.

use chrono::{FixedOffset, TimeZone, Timelike};
use myharness_sessions::{
    CodeRange, InMemoryAuditKeys, SessionService, SessionError,
};
use myharness_store::{NewSession, NewUsageEvent, Store, TurnRole, TurnStatus};
use tempfile::TempDir;

fn fixture() -> (TempDir, SessionService<InMemoryAuditKeys>) {
    let dir = TempDir::new().expect("temp dir");
    let store = Store::open(&dir.path().join("code.db"), &dir.path().join("files"))
        .expect("open store");
    (dir, SessionService::new(store, InMemoryAuditKeys::new()))
}

fn start(service: &SessionService<InMemoryAuditKeys>, surface: &str, title: &str) -> String {
    service
        .start(NewSession {
            project_id: None,
            surface,
            title,
            ..Default::default()
        })
        .expect("start session")
        .id
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
}

/// A fixed timestamp keeps the empty/capped-window assertions deterministic.
const BASE_MS: i64 = 1_700_000_000_000;

fn days_ago(base: i64, days: i64) -> i64 {
    base - days * 24 * 60 * 60 * 1000
}

#[test]
fn code_summary_excludes_cowork_and_respects_ranges() {
    let (_dir, service) = fixture();
    let code = start(&service, "code", "code work");
    let chat = start(&service, "chat", "chat work");
    let base = now_ms();
    let current = base - 60 * 60 * 1000;

    service
        .store()
        .record_usage_at(
            &code,
            NewUsageEvent {
                model: "model-b",
                input_tokens: 10,
                output_tokens: 5,
                ..Default::default()
            },
            current,
        )
        .expect("current code usage");
    service
        .store()
        .record_usage_at(
            &code,
            NewUsageEvent {
                model: "model-a",
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            days_ago(base, 40),
        )
        .expect("old code usage");
    service
        .store()
        .record_usage_at(
            &chat,
            NewUsageEvent {
                model: "chat-model",
                input_tokens: 999,
                output_tokens: 999,
                ..Default::default()
            },
            base,
        )
        .expect("chat usage");

    service
        .append_turn(&code, 0, TurnRole::User, TurnStatus::Complete, b"{}", None, None, None)
        .expect("code user turn");
    service
        .append_turn(
            &code,
            1,
            TurnRole::Assistant,
            TurnStatus::Complete,
            b"{}",
            None,
            None,
            None,
        )
        .expect("code assistant turn");
    service
        .append_turn(&code, 2, TurnRole::Tool, TurnStatus::Complete, b"{}", None, None, None)
        .expect("code tool turn");
    service
        .append_turn(&chat, 0, TurnRole::User, TurnStatus::Complete, b"{}", None, None, None)
        .expect("chat user turn");

    let tz = FixedOffset::east_opt(0).expect("offset");
    let now = now_ms();

    let seven = service
        .code_usage_summary(CodeRange::SevenDays, tz, now)
        .expect("7d summary");
    assert_eq!(seven.days, 7);
    assert_eq!(seven.totals.events, 1);
    assert_eq!(seven.totals.input_tokens, 10);
    assert_eq!(seven.totals.output_tokens, 5);
    assert_eq!(seven.sessions, 1);
    assert_eq!(seven.messages, 2);
    assert!(seven.active_days >= 1);
    let expected_hour = tz
        .timestamp_millis_opt(current)
        .single()
        .expect("local time")
        .hour();
    assert_eq!(seven.peak_hour, Some(expected_hour));
    assert_eq!(seven.favorite_model.as_deref(), Some("model-b"));
    assert_eq!(seven.models.len(), 1);
    assert_eq!(seven.models[0].model, "model-b");
    assert!((seven.models[0].share_percent - 100.0).abs() < f64::EPSILON);

    let thirty = service
        .code_usage_summary(CodeRange::ThirtyDays, tz, now)
        .expect("30d summary");
    assert_eq!(thirty.days, 30);
    assert_eq!(thirty.totals.events, 1);
    assert_eq!(thirty.messages, 2);

    let all = service
        .code_usage_summary(CodeRange::All, tz, now)
        .expect("all summary");
    assert_eq!(all.range, CodeRange::All);
    let expected_all_days = (tz
        .timestamp_millis_opt(now)
        .single()
        .expect("local now")
        .date_naive()
        - tz
            .timestamp_millis_opt(days_ago(base, 40))
            .single()
            .expect("local old")
            .date_naive())
        .num_days()
        + 1;
    assert_eq!(all.days, expected_all_days as u32);
    assert!(!all.truncated);
    assert_eq!(all.totals.events, 2);
    assert_eq!(all.totals.input_tokens, 110);
    assert_eq!(all.totals.output_tokens, 55);
    assert_eq!(all.sessions, 1);
    assert_eq!(all.messages, 2);
    assert!(all.active_days >= 1);
    assert_eq!(all.favorite_model.as_deref(), Some("model-a"));
    assert_eq!(all.models.len(), 2);
    assert_eq!(all.models[0].model, "model-a");
    assert!((all.models[0].share_percent - (150.0 / 165.0 * 100.0)).abs() < 1e-9);
    assert_eq!(all.daily.len(), all.days as usize);
}

#[test]
fn an_empty_ledger_reports_a_real_empty_window() {
    let (_dir, service) = fixture();
    let tz = FixedOffset::east_opt(0).expect("offset");
    let summary = service
        .code_usage_summary(CodeRange::All, tz, BASE_MS)
        .expect("empty summary");
    assert_eq!(summary.days, 1);
    assert!(!summary.truncated);
    assert_eq!(summary.totals.events, 0);
    assert_eq!(summary.sessions, 0);
    assert_eq!(summary.messages, 0);
    assert_eq!(summary.active_days, 0);
    assert_eq!(summary.peak_hour, None);
    assert_eq!(summary.favorite_model, None);
    assert!(summary.models.is_empty());
    assert_eq!(summary.daily.len(), 1);
}

#[test]
fn all_time_counts_a_turn_written_before_the_first_usage_event() {
    let (_dir, service) = fixture();
    let code = start(&service, "code", "first turn");
    service
        .append_turn(&code, 0, TurnRole::User, TurnStatus::Complete, b"{}", None, None, None)
        .expect("first user turn");
    // A real turn writes its user row before the provider finishes and the
    // usage event is recorded. All-time must not start at the later event and
    // silently drop that first message.
    let after_turn = now_ms();
    service
        .store()
        .record_usage_at(
            &code,
            NewUsageEvent {
                model: "model-a",
                input_tokens: 3,
                output_tokens: 2,
                ..Default::default()
            },
            after_turn + 1_000,
        )
        .expect("usage after the turn");

    let tz = FixedOffset::east_opt(0).expect("offset");
    let summary = service
        .code_usage_summary(CodeRange::All, tz, after_turn + 2_000)
        .expect("all-time summary");
    assert_eq!(summary.totals.events, 1);
    assert_eq!(summary.sessions, 1);
    assert_eq!(summary.messages, 1);
}

#[test]
fn an_all_time_series_older_than_the_cap_is_marked_truncated() {
    let (_dir, service) = fixture();
    let code = start(&service, "code", "old work");
    let base = BASE_MS;
    service
        .store()
        .record_usage_at(
            &code,
            NewUsageEvent {
                model: "old-model",
                input_tokens: 1,
                output_tokens: 1,
                ..Default::default()
            },
            days_ago(base, 4000),
        )
        .expect("old usage");
    let tz = FixedOffset::east_opt(0).expect("offset");
    let summary = service
        .code_usage_summary(CodeRange::All, tz, BASE_MS)
        .expect("capped summary");
    assert!(summary.truncated);
    assert_eq!(summary.days, 3660);
    assert_eq!(summary.daily.len(), 3660);
    // The strip cannot grow without bound, but "all time" totals still include
    // the pre-cap event. The flag describes the strip, not the totals.
    assert_eq!(summary.totals.events, 1);
    assert_eq!(summary.totals.input_tokens, 1);
    assert_eq!(summary.sessions, 1);
    assert_eq!(summary.active_days, 1);
    assert_eq!(summary.favorite_model.as_deref(), Some("old-model"));
    assert!(summary.daily.iter().all(|day| day.totals.events == 0));
}

#[test]
fn an_invalid_window_is_refused() {
    let (_dir, service) = fixture();
    let tz = FixedOffset::east_opt(0).expect("offset");
    let err = service
        .code_usage_summary(CodeRange::All, tz, i64::MAX)
        .expect_err("out-of-range time is invalid");
    assert!(matches!(err, SessionError::Store(_)));
}
