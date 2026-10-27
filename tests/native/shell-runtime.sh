#!/usr/bin/env bash
# Native-shell runtime fixture — Phase 1 (docs/13 §4).
#
# Exercised here: TEST-ARCH-002 single-instance second-launch routing, and the
# deep-link plumbing of docs/02 §2 step 2 / docs/06 §6 (argv on cold start and on a
# second launch; fail-closed classification per docs/06 §1 route table, TEST-SEC-003;
# ARCH-NAV-01 deep-linkable routes; ARCH-NAV-03 Setup3p opens the dedicated
# setup window (Phase 2), and the main renderer is never navigated to it).
#
# Observable: debug builds trace `[myharness] …` lines to stderr (release silent).
# Run: bash tests/native/shell-runtime.sh
# Requires: the built debug binary (cargo build in apps/desktop/src-tauri) and a
# graphical session. Kills its own instances; leaves no state behind.
set -u

BIN=/home/manjula/myharness-rust/target/debug/myharness-desktop
DIST=/home/manjula/myharness-rust/apps/desktop/dist
LOG=/tmp/opencode/myharness-fixture.log

# The log directory is not the script's own; a missing parent makes every
# launch's redirect fail silently and every case time out waiting on a log
# that can never appear.
mkdir -p "$(dirname "$LOG")"

pass=0; fail=0

launch() { # launch <args...>  -> primary in background, logs to $LOG
  : > "$LOG"
  MYHARNESS_DIST="$DIST" "$BIN" "$@" > "$LOG" 2>&1 &
  PRIMARY=$!
}

wait_log() { # wait_log <pattern> <timeout-seconds>
  local pattern="$1" deadline=$(( SECONDS + ${2:-10} ))
  until grep -q "$pattern" "$LOG" 2>/dev/null; do
    [ "$SECONDS" -ge "$deadline" ] && return 1
    sleep 0.3
  done
  return 0
}

check() { # check <name>  (+ pass/fail via exit code of caller assertion)
  if [ "$?" -eq 0 ]; then pass=$((pass+1)); echo "PASS $1"; else fail=$((fail+1)); echo "FAIL $1"; fi
}

is_up() {
  sleep 4
  kill -0 "$PRIMARY" 2>/dev/null || return 1
  pgrep -f WebKitWebProcess >/dev/null || return 1
  return 0
}

second_exit_time_lt() { # second_exit_time_lt <args...> <max-seconds>
  local max="$1"; shift
  local start=$SECONDS
  MYHARNESS_DIST="$DIST" "$BIN" "$@" >/dev/null 2>&1
  local code=$?
  [ "$code" -eq 0 ] || { echo "second exit code $code"; return 1; }
  [ $((SECONDS - start)) -lt "$max" ] && return 0
  echo "second took $((SECONDS - start))s"; return 1
}

# R1 — cold boot stays up with a live webview AND a mounted SPA (window + app://
# pass-through branch). The renderer handshake is the rendered-content proof:
# a load-error page never executes the bundle that invokes `renderer_ready`.
launch
is_up; check "R1 cold boot: process + webview child alive" || true
wait_log "renderer ready" 10
check "R1 cold boot: renderer mounted (not a load-error page)" || true

# R2 — second launch (no args) exits fast; primary persists (docs/02 §2 step 2).
( second_exit_time_lt 12 >/dev/null 2>&1 ) & SUB=$!
wait "$SUB"; check "R2 second launch exits fast with code 0" || true
kill -0 "$PRIMARY" 2>/dev/null; check "R2 primary persists after second launch" || true

# R3 — deep link via second launch navigates the primary (fail-open on classification
# happens in the primary's log: observed acceptance line + no secondary window ever).
( second_exit_time_lt 12 "myharness://projects" >/dev/null 2>&1 ) & SUB=$!
wait "$SUB"; check "R3 second launch with deep link exits fast" || true
wait_log "deep-link accepted myharness://projects" 6
check "R3 primary navigated to /projects (log proof)" || true
kill -0 "$PRIMARY" 2>/dev/null; check "R3 primary persists" || true

# R4 — unknown scheme and unknown route are rejected, window unaffected (TEST-SEC-003).
OFF4=$(wc -l < "$LOG") # assert only what this launch appends (R3 accepted earlier)
( second_exit_time_lt 12 "claude://new" "myharness://delete-everything" >/dev/null 2>&1 ) & SUB=$!
wait "$SUB"; check "R4 second launch with hostile argv exits fast" || true
wait_log "deep-link rejected claude://new" 6
check "R4 unknown scheme rejected" || true
grep -q "deep-link rejected myharness://delete-everything" "$LOG"
check "R4 unknown route rejected" || true
if tail -n +$((OFF4 + 1)) "$LOG" | grep -q "deep-link accepted"; then false; else true; fi
check "R4 no navigation accepted for hostile argv" || true
kill -0 "$PRIMARY" 2>/dev/null; check "R4 primary persists" || true

# Kill R1..R4 primary; cold-start cases follow.
kill "$PRIMARY" 2>/dev/null; wait "$PRIMARY" 2>/dev/null

# R5 — cold start WITH a deep link navigates at boot (argv deep link, docs/06 §6).
launch "myharness://scheduled-task"
wait_log "deep-link accepted myharness://scheduled-task -> app://localhost/scheduled-task" 8
check "R5 cold-start deep link accepted and navigated" || true
is_up; check "R5 window alive after cold-start deep link" || true
kill "$PRIMARY" 2>/dev/null; wait "$PRIMARY" 2>/dev/null

# R6 — cold start with ONLY an invalid deep link: fail closed, boot unaffected.
launch "myharness://bogus-rute"
wait_log "deep-link rejected myharness://bogus-rute" 8
check "R6 invalid cold-start deep link rejected in log" || true
is_up; check "R6 window still opens and stays up (fail closed)" || true
# Setup3p: recognized grammar; the DEDICATED setup window opens (Phase 2,
# ARCH-NAV-03) and the main renderer is still not navigated to it.
kill "$PRIMARY" 2>/dev/null; wait "$PRIMARY" 2>/dev/null
launch "myharness://setup-desktop-3p"
wait_log "setup-desktop-3p: opening the dedicated setup window" 8
check "R7 Setup3p grammar recognized, dedicated window requested" || true
wait_log "setup window opened" 8
check "R7 dedicated setup window created (log proof)" || true
if grep -q "deep-link accepted myharness://setup-desktop-3p" "$LOG"; then false; else true; fi
check "R7 main renderer not navigated to the setup route" || true
is_up; check "R7 window alive with Setup3p link" || true

kill "$PRIMARY" 2>/dev/null; wait "$PRIMARY" 2>/dev/null
rm -f "$LOG"

echo ""
echo "NATIVE SUMMARY: $((pass+fail)) checks — PASS $pass / FAIL $fail"
[ "$fail" -eq 0 ]
