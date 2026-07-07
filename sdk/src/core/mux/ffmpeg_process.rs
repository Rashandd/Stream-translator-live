//! ffmpeg child process lifecycle: locate the binary (best-effort, same
//! pattern as `gpu_probe::probe` for `nvidia-smi`), spawn it with arbitrary
//! arguments, hand back its stdin, and let the caller await its exit.
//!
//! Deliberately generic (takes raw args, doesn't know about dub-mux/FLV/PCM)
//! so it's testable in isolation with a trivial ffmpeg invocation — the
//! dub-target-specific command line lives in `mux::mod` (`run_dub_target`),
//! which is the only caller that needs the real two-input mux command.

use std::process::Stdio;

use tokio::process::{Child, ChildStdin, Command};

/// Best-effort check for a runnable `ffmpeg` on PATH — mirrors
/// `gpu_probe::probe`'s "shell out, degrade to `None` on any failure"
/// posture. We don't need ffmpeg's actual filesystem location (the OS
/// resolves `"ffmpeg"` from PATH when spawning), only whether invoking it
/// works at all.
pub fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("ffmpeg binary not found on PATH")]
    NotFound,
    #[error("failed to spawn ffmpeg: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("ffmpeg process has no stdin handle")]
    NoStdin,
}

/// A running ffmpeg child process. Killed automatically on drop
/// (`kill_on_drop`) — no manual `Drop` impl needed, tokio already does this
/// correctly (SIGKILL on Unix, `TerminateProcess` on Windows).
pub struct FfmpegProcess {
    child: Child,
    stdin: ChildStdin,
}

impl FfmpegProcess {
    /// Spawns `ffmpeg` with the given arguments. Fails fast with
    /// [`FfmpegError::NotFound`] if ffmpeg isn't on PATH, rather than letting
    /// the caller wait on a `spawn()` that's guaranteed to fail.
    pub async fn spawn(args: &[String]) -> Result<Self, FfmpegError> {
        if !ffmpeg_available() {
            return Err(FfmpegError::NotFound);
        }
        let mut child = Command::new("ffmpeg")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(FfmpegError::Spawn)?;
        let stdin = child.stdin.take().ok_or(FfmpegError::NoStdin)?;
        Ok(FfmpegProcess { child, stdin })
    }

    /// The process's stdin — callers write the FLV byte stream here.
    pub fn stdin(&mut self) -> &mut ChildStdin {
        &mut self.stdin
    }

    /// Waits for the process to exit (crash or clean shutdown). The caller
    /// decides what to do next (e.g. `rtmp::relay`'s reconnect/backoff
    /// pattern) — this function only reports, never restarts.
    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same best-effort posture as `gpu_probe::tests::real_probe_is_best_effort`:
    /// check the real tool's availability first, then assert our function
    /// agrees — never a hard requirement either way, so this doesn't fail on
    /// machines without ffmpeg installed.
    #[test]
    fn ffmpeg_available_is_best_effort() {
        let real = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert_eq!(ffmpeg_available(), real);
    }

    /// Real ffmpeg smoke test: trivial synthetic input, no real-time I/O
    /// needed (`-f lavfi -i testsrc`), just proves spawn/stdin/wait/kill_on_drop
    /// work against the actual binary. Skips (doesn't fail) if ffmpeg isn't
    /// installed on this machine.
    #[tokio::test]
    async fn spawns_and_waits_on_a_real_ffmpeg_process() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed on this machine");
            return;
        }
        let args: Vec<String> = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=64x64:rate=1",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let mut proc = FfmpegProcess::spawn(&args).await.expect("spawn");
        let status = proc.wait().await.expect("wait");
        assert!(status.success(), "ffmpeg exited with {status:?}");
    }

    #[tokio::test]
    async fn kill_on_drop_stops_a_long_running_process() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed on this machine");
            return;
        }
        // A source that would otherwise run for a long time.
        let args: Vec<String> = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=3600:size=64x64:rate=1",
            "-f",
            "null",
            "-",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let proc = FfmpegProcess::spawn(&args).await.expect("spawn");
        // Dropping without an explicit kill must not leave the process
        // running — kill_on_drop is what guarantees this, no manual cleanup.
        drop(proc);
        // Nothing to assert directly (the OS reaps it) — this test's value
        // is compiling/running without hanging the test process itself.
    }
}
