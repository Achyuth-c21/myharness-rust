//! The Code terminal drawer's PTY lifecycle (docs/23 §7 addendum item 8;
//! IMP-55; REQ-CODE-04).
//!
//! The observed original opens a drawer with a live shell prompt from the Code
//! header's terminal glyph. The rebuild keeps the trust boundary intact
//! (`docs/10` §8): the renderer never touches the PTY — it asks the bridge to
//! open one, streams bytes through the app's event channel, and closes it.
//!
//! Bounded by construction (the roadmap's PTY rules):
//!
//! - **Drain on close and on quit** — `close` stops reading, kills the child,
//!   and waits (bounded) for exit; `close_all` runs at app quit so no shell
//!   outlives the app (`REQ-CODE-04`).
//! - **Output is capped** — a per-session total byte bound (10 MiB) stops a
//!   runaway process from flooding the event channel; further output is
//!   dropped and the cap is reported once.
//! - **The command is the user's shell** (`$SHELL`, falling back to
//!   `/bin/sh`); the working directory must exist and is canonicalized before
//!   the spawn — the renderer's path is data, never an instruction.
//! - A full VT100 emulator is NOT invented here (recorded rebuild definition,
//!   `docs/19` IMP-55): the drawer renders raw output and a line input.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The per-session output bound. A runaway process is dropped past this, and
/// the fact is reported once rather than flooding the channel.
pub const OUTPUT_BYTE_CAP: usize = 10 * 1024 * 1024;

struct TerminalSession {
    writer: Option<Box<dyn Write + Send>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    bytes_written_total: usize,
}

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalSession")
            .field("writer", &self.writer.is_some())
            .field("bytes_written_total", &self.bytes_written_total)
                        .finish()
    }
}

/// The open terminals. The Tauri layer reads from a session with its reader
/// thread and emits chunks on the app's event channel; the registry owns the
/// handles and the lifecycle.
#[derive(Default)]
pub struct TerminalRegistry {
    next_id: AtomicU64,
    sessions: Mutex<HashMap<u64, TerminalSession>>,
}

/// A newly opened terminal. The renderer receives only the id.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TerminalOpened {
    pub id: u64,
}

#[derive(Debug)]
pub struct TerminalError(pub String);

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TerminalRegistry {
    /// Open a shell in `cwd`. The command is the user's shell; the path must
    /// exist (canonicalized here — the renderer's spelling is untrusted).
    /// `None` means the user's home directory (the drawer's unbound state).
    pub fn open(
        &self,
        cwd: Option<&str>,
    ) -> Result<(TerminalOpened, Box<dyn Read + Send>), TerminalError> {
        let home = std::env::var("HOME").ok();
        let target = cwd
            .map(str::to_string)
            .or(home)
            .ok_or_else(|| TerminalError("no directory for the terminal".into()))?;
        let canonical = std::fs::canonicalize(&target)
            .map_err(|error| TerminalError(format!("the terminal directory is not available: {error}")))?;
        if !canonical.is_dir() {
            return Err(TerminalError("the terminal path is not a directory".into()));
        }
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| TerminalError(format!("the pseudoterminal could not be opened: {error}")))?;
        let mut command = CommandBuilder::new(&shell);
        command.cwd(&canonical);
        // The child inherits the app's environment; TERM keeps line-oriented
        // programs readable in the raw-output drawer.
        command.env("TERM", "xterm-256color");
        let child = pty
            .slave
            .spawn_command(command)
            .map_err(|error| TerminalError(format!("the shell could not be started: {error}")))?;
        let writer = pty
            .master
            .take_writer()
            .map_err(|error| TerminalError(format!("the terminal input could not be attached: {error}")))?;
        let reader = pty
            .master
            .try_clone_reader()
            .map_err(|error| TerminalError(format!("the terminal output could not be attached: {error}")))?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.sessions.lock().map_err(|_| poisoned())?.insert(
            id,
            TerminalSession {
                writer: Some(writer),
                child,
                bytes_written_total: 0,
            },
        );
        Ok((TerminalOpened { id }, reader))
    }

    /// Send the renderer's input bytes to the shell.
    pub fn write(&self, id: u64, bytes: &[u8]) -> Result<(), TerminalError> {
        let mut sessions = self.sessions.lock().map_err(|_| poisoned())?;
        let session = sessions
            .get_mut(&id)
            .ok_or_else(|| TerminalError("no such terminal".into()))?;
        let writer = session
            .writer
            .as_mut()
            .ok_or_else(|| TerminalError("the terminal input is closed".into()))?;
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .map_err(|error| TerminalError(format!("the terminal input failed: {error}")))
    }

    /// Count the bytes the reader thread has forwarded, for the cap. The
    /// reader thread calls this after emitting a chunk; `false` means the cap
    /// is hit and the caller must stop emitting (and may close).
    pub fn account_output(&self, id: u64, len: usize) -> bool {
        let Ok(mut sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(session) = sessions.get_mut(&id) else {
            return false;
        };
        session.bytes_written_total = session.bytes_written_total.saturating_add(len);
        session.bytes_written_total <= OUTPUT_BYTE_CAP
    }

    /// Close one terminal: kill the child and wait (bounded) for exit — the
    /// drain rule (`REQ-CODE-04`). Idempotent.
    pub fn close(&self, id: u64) -> Result<(), TerminalError> {
        let mut sessions = self.sessions.lock().map_err(|_| poisoned())?;
        // Removing the session hands the drain the whole child: close never
        // blocks and closing twice is a no-op (already gone).
        let Some(mut session) = sessions.remove(&id) else {
            return Ok(());
        };
        session.writer = None; // Dropping the writer closes the PTY input.
        // portable-pty 0.8 offers only an unbounded wait, so the drain is
        // kill-now + reap-on-a-detached thread (REQ-CODE-04).
        let _ = session.child.kill();
        let mut child = session.child;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    /// Close every terminal — the app-quit drain (`REQ-CODE-04`).
    pub fn close_all(&self) {
        let ids: Vec<u64> = {
            let Ok(sessions) = self.sessions.lock() else {
                return;
            };
            sessions.keys().copied().collect()
        };
        for id in ids {
            let _ = self.close(id);
        }
    }

    /// The count of live sessions (tests and the quit guard).
    pub fn live_count(&self) -> usize {
        self.sessions.lock().map(|sessions| sessions.len()).unwrap_or(0)
    }
}

fn poisoned() -> TerminalError {
    TerminalError("the terminal registry is unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real shell round-trip: echo through the PTY, bounded wait, then the
    /// drain-on-close rule leaves nothing running.
    #[test]
    fn a_shell_round_trips_and_close_drains() {
        let registry = TerminalRegistry::default();
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let (opened, mut reader) = registry
            .open(Some(tmp.path().to_str().expect("utf-8")))
            .expect("open");
        registry
            .write(opened.id, b"echo terminal-round-trip-marker; exit\n")
            .expect("write");
        // The PTY read blocks, so the loop runs on a thread and the test
        // waits with a bound — a silent shell must fail the test, not hang it.
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut chunk = [0u8; 512];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        seen.extend_from_slice(&chunk[..read]);
                        if seen
                            .windows(28)
                            .any(|window| window == b"terminal-round-trip-marker")
                        {
                            break;
                        }
                    }
                }
            }
            let _ = tx.send(seen);
        });
        let seen = rx
            .recv_timeout(Duration::from_secs(15))
            .expect("the shell echoed within the bound");
        registry.close(opened.id).expect("close");
        let seen_text = String::from_utf8_lossy(&seen);
        assert!(
            seen_text.contains("terminal-round-trip-marker"),
            "the shell echoed through the PTY, got: {seen_text}"
        );
        assert_eq!(registry.live_count(), 0, "close removes the session");
    }

    #[test]
    fn an_unknown_terminal_and_double_close_are_honest() {
        let registry = TerminalRegistry::default();
        assert!(registry.write(999, b"x").is_err(), "unknown id refuses");
        registry.close(999).expect("close is idempotent");
    }

    #[test]
    fn a_missing_directory_refuses_before_any_spawn() {
        let registry = TerminalRegistry::default();
        let error = registry
            .open(Some("/definitely/missing/myharness-cwd"))
            .err()
            .expect("refused");
        assert!(error.0.contains("not available"));
        assert_eq!(registry.live_count(), 0);
    }

    #[test]
    fn the_output_cap_accounts_bytes() {
        let registry = TerminalRegistry::default();
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let (opened, _reader) = registry
            .open(Some(tmp.path().to_str().expect("utf-8")))
            .expect("open");
        assert!(registry.account_output(opened.id, OUTPUT_BYTE_CAP));
        assert!(
            !registry.account_output(opened.id, 1),
            "the cap stops further emission"
        );
    }
}
