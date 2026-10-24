/**
 * Settings → Usage (docs/screens/settings.md §Usage; `TEST-SESSION-008`,
 * `TEST-DATA-007`).
 *
 * The range selector is 7/30/90 days; totals are device-local and never
 * presented as billing. The ledger stores raw provider events, so this panel
 * reads the bridge's local-day aggregation with the device's UTC offset — an
 * event at 23:30 local stays on that local day even after UTC has rolled over.
 *
 * Cached tokens are shown separately: docs/11 §6 records that they are excluded
 * from token limits. The Phase-4 ledger has no surface column, so Cowork and
 * Code are shown together; the per-surface split stays explicitly unbuilt
 * rather than guessed.
 */
import { useEffect, useState } from "react";
import {
  chat,
  type BridgeError,
  type UsageSummary,
  type UsageTotals,
} from "../chat/client";

const RANGES = [7, 30, 90] as const;

function formatCount(value: number): string {
  return value.toLocaleString();
}

function Totals({ totals }: { totals: UsageTotals }) {
  return (
    <div className="usage-totals">
      <div className="usage-total">
        <span className="usage-label">Input</span>
        <span className="usage-value">{formatCount(totals.inputTokens)}</span>
      </div>
      <div className="usage-total">
        <span className="usage-label">Output</span>
        <span className="usage-value">{formatCount(totals.outputTokens)}</span>
      </div>
      <div className="usage-total">
        <span className="usage-label">Cache read</span>
        <span className="usage-value">{formatCount(totals.cacheReadTokens)}</span>
      </div>
      <div className="usage-total">
        <span className="usage-label">Cache write</span>
        <span className="usage-value">{formatCount(totals.cacheWriteTokens)}</span>
      </div>
      {totals.searchRequests > 0 ? (
        <div className="usage-total">
          <span className="usage-label">Search requests</span>
          <span className="usage-value">{formatCount(totals.searchRequests)}</span>
        </div>
      ) : null}
      <div className="usage-total">
        <span className="usage-label">Turns</span>
        <span className="usage-value">{formatCount(totals.events)}</span>
      </div>
      {totals.errorEvents > 0 ? (
        <div className="usage-total">
          <span className="usage-label">Failed turns</span>
          <span className="usage-value">{formatCount(totals.errorEvents)}</span>
        </div>
      ) : null}
    </div>
  );
}

function DailyChart({ summary }: { summary: UsageSummary }) {
  const peak = Math.max(
    1,
    ...summary.daily.map((day) => day.totals.inputTokens + day.totals.outputTokens),
  );
  const first = summary.daily[0]?.date ?? "";
  const last = summary.daily[summary.daily.length - 1]?.date ?? "";
  return (
    <div className="usage-chart-wrap">
      <div
        className="usage-chart"
        role="img"
        aria-label={`Daily input and output tokens from ${first} to ${last}`}
      >
        {summary.daily.map((day) => {
          const tokens = day.totals.inputTokens + day.totals.outputTokens;
          return (
            <div
              key={day.date}
              className="usage-bar-slot"
              title={`${day.date}: ${formatCount(tokens)} tokens`}
            >
              <div
                className="usage-bar"
                style={{ height: `${Math.max(tokens > 0 ? 3 : 0, (tokens / peak) * 100)}%` }}
              />
            </div>
          );
        })}
      </div>
      <div className="usage-axis">
        <span>{first}</span>
        <span>{last}</span>
      </div>
    </div>
  );
}

export function UsagePanel() {
  const [days, setDays] = useState<(typeof RANGES)[number]>(7);
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      // `getTimezoneOffset` is minutes west of UTC; the bridge wants east.
      const utcOffsetMinutes = -new Date().getTimezoneOffset();
      const response = await chat({
        type: "usageSummary",
        days,
        utcOffsetMinutes,
      });
      if (cancelled) return;
      setLoading(false);
      if (response.type === "usage") {
        setSummary(response.summary);
        setError(null);
      } else if (response.type === "error") {
        setSummary(null);
        setError(response.error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [days]);

  return (
    <div className="usage-panel">
      <div className="usage-range" role="group" aria-label="Usage range">
        {RANGES.map((range) => (
          <button
            key={range}
            type="button"
            className={`usage-range-btn${range === days ? " active" : ""}`}
            aria-pressed={range === days}
            onClick={() => setDays(range)}
          >
            {range}d
          </button>
        ))}
      </div>

      {loading ? (
        <p className="settings-note" role="status">
          Reading the local ledger…
        </p>
      ) : null}

      {!loading && error ? (
        <p className="settings-note" role="alert">
          {error.code === "BRIDGE_UNAVAILABLE"
            ? "Usage is available in the desktop app. This browser preview has no local ledger."
            : error.message}
        </p>
      ) : null}

      {!loading && summary ? (
        <>
          <h3>Tokens</h3>
          <Totals totals={summary.totals} />
          <h3>Daily</h3>
          <DailyChart summary={summary} />
          <p className="settings-note">
            Values are device-local and are not a billing statement. Cached tokens are shown
            separately and are excluded from token limits. The Phase-4 ledger records no surface
            per event, so Cowork and Code totals are combined until that column exists.
          </p>
        </>
      ) : null}
    </div>
  );
}
