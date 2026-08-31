//! The process adapter: the obligations a wrapper owes its child.
//!
//! Both output pipes are drained concurrently while stdin is being written,
//! because filling a pipe buffer while the child blocks on stdin is a
//! deadlock. The environment is constructed by the caller and applied over
//! `env_clear`. A child killed by signal N surfaces as 128+N, so an
//! interrupted setup dies the way an operator expects. Interruption reaches
//! the child through the shared process group — a terminal's SIGINT is
//! delivered to parent and child alike — and this adapter installs no
//! handler of its own: forwarding a signal aimed at `rk` alone would need a
//! raw `kill(2)`, which the crate-wide `unsafe_code = "forbid"` rules out.
//! Chunks reach the caller in arrival order, raw; redaction is the caller's
//! job, because only the caller knows the secrets of the run.

use std::ffi::OsString;
use std::io::{Read, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;

use zeroize::Zeroizing;

use crate::events::ChildStream;

/// One spawn request, fully constructed before anything runs.
pub struct Exec {
    /// The program to spawn.
    pub program: OsString,
    /// Its arguments; a secret never appears here.
    pub args: Vec<OsString>,
    /// The constructed environment, applied over `env_clear`.
    pub env: Vec<(OsString, OsString)>,
    /// The working directory.
    pub cwd: PathBuf,
    /// Bytes written to the child's stdin, then closed. Scrubbed on drop,
    /// because this is the channel a credential travels on.
    pub stdin: Option<Zeroizing<Vec<u8>>>,
}

impl std::fmt::Debug for Exec {
    /// Everything but `stdin`: that field carries a credential, and a
    /// derived rendering would put it in whatever formatted an `Exec`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exec")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("stdin", &self.stdin.as_ref().map(|_| "[redacted]"))
            .finish_non_exhaustive()
    }
}

impl Exec {
    /// The one `+ `-prefixed echo line: a rendering of the typed argument
    /// list, produced here and never by shell tracing.
    #[must_use]
    pub fn echo(&self) -> String {
        let mut line = String::from("+ ");
        line.push_str(&self.program.to_string_lossy());
        for arg in &self.args {
            line.push(' ');
            line.push_str(&arg.to_string_lossy());
        }
        line
    }
}

/// What a finished child left behind.
#[derive(Debug)]
pub struct Outcome {
    /// The surfaced exit code: the child's own, or 128+N for signal N.
    pub exit_code: i32,
    /// Everything the child wrote to stdout, in order.
    pub stdout: Vec<u8>,
    /// Everything the child wrote to stderr, in order.
    pub stderr: Vec<u8>,
}

impl Outcome {
    /// Whether the child succeeded.
    #[must_use]
    pub const fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Run a child to completion, draining both pipes concurrently and calling
/// `on_chunk` for every chunk in arrival order.
///
/// # Errors
///
/// Returns the spawn failure; a child that runs and fails is an [`Outcome`],
/// not an error.
pub fn run(exec: &Exec, mut on_chunk: impl FnMut(ChildStream, &[u8])) -> std::io::Result<Outcome> {
    let mut command = Command::new(&exec.program);
    command
        .args(&exec.args)
        .env_clear()
        .envs(exec.env.iter().map(|(k, v)| (k, v)))
        .current_dir(&exec.cwd)
        .stdin(if exec.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;

    let (sender, receiver) = mpsc::channel::<(ChildStream, Vec<u8>)>();
    let mut drains = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        drains.push(spawn_drain(pipe, ChildStream::Stdout, sender.clone()));
    }
    if let Some(pipe) = child.stderr.take() {
        drains.push(spawn_drain(pipe, ChildStream::Stderr, sender));
    }

    // The drains are already running, so this write cannot deadlock against
    // a full output pipe; a child that exits early surfaces as a broken
    // pipe, which only means it stopped reading.
    if let Some(bytes) = &exec.stdin {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(bytes);
        }
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for (stream, chunk) in receiver {
        on_chunk(stream, &chunk);
        match stream {
            ChildStream::Stdout => stdout.extend_from_slice(&chunk),
            ChildStream::Stderr => stderr.extend_from_slice(&chunk),
        }
    }
    for drain in drains {
        let _ = drain.join();
    }
    let status = child.wait()?;
    Ok(Outcome {
        exit_code: surface_exit(status),
        stdout,
        stderr,
    })
}

/// Drain one pipe to the channel in chunks, preserving arrival order within
/// the stream and byte fidelity across invalid UTF-8.
fn spawn_drain(
    mut pipe: impl Read + Send + 'static,
    stream: ChildStream,
    sender: mpsc::Sender<(ChildStream, Vec<u8>)>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if sender.send((stream, buffer[..n].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// The exit code a caller sees: the child's own, or 128+N for signal N.
fn surface_exit(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    -1
}

/// Replace every occurrence of each secret in `chunk` with `[redacted]`.
///
/// Chunk-level replacement is the guarantee the tests hold; a secret split
/// exactly across a chunk boundary is out of reach here, which is one more
/// reason no step ever prints one.
#[must_use]
pub fn redact(chunk: &[u8], secrets: &[impl AsRef<[u8]>]) -> Vec<u8> {
    let mut out = chunk.to_vec();
    for secret in secrets {
        let secret = secret.as_ref();
        if secret.is_empty() {
            continue;
        }
        while let Some(pos) = out
            .windows(secret.len())
            .position(|window| window == secret)
        {
            out.splice(pos..pos + secret.len(), b"[redacted]".iter().copied());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Exec, Zeroizing, redact, run};
    use std::path::PathBuf;

    fn sh(script: &str, stdin: Option<Vec<u8>>) -> Exec {
        Exec {
            program: "sh".into(),
            args: vec!["-c".into(), script.into()],
            env: vec![(
                "PATH".into(),
                std::env::var_os("PATH").expect("a PATH exists"),
            )],
            cwd: PathBuf::from("."),
            stdin: stdin.map(Zeroizing::new),
        }
    }

    /// A formatted spawn request never carries what it writes to stdin.
    #[test]
    fn a_debug_rendering_omits_the_stdin_bytes() {
        let exec = sh("true", Some(b"sekret-stdin-value".to_vec()));
        let rendered = format!("{exec:?}");
        assert!(!rendered.contains("sekret-stdin-value"));
        assert!(rendered.contains("[redacted]"));
    }

    /// The concurrent-drain obligation: output far past the pipe buffer
    /// completes while stdin is being written.
    #[test]
    fn a_chatty_child_with_stdin_does_not_deadlock() {
        let big_input = vec![b'x'; 512 * 1024];
        let exec = sh(
            "cat >/dev/null; i=0; while [ $i -lt 300 ]; do printf '%01024d' $i; printf '%0512d' $i >&2; i=$((i+1)); done",
            Some(big_input),
        );
        let outcome = run(&exec, |_, _| {}).expect("the child runs");
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.len(), 300 * 1024);
        assert_eq!(outcome.stderr.len(), 300 * 512);
    }

    /// Invalid UTF-8 travels byte-for-byte.
    #[test]
    fn invalid_utf8_is_preserved() {
        let exec = sh(r"printf 'a\377\376b'", None);
        let outcome = run(&exec, |_, _| {}).expect("the child runs");
        assert_eq!(outcome.stdout, [b'a', 0xff, 0xfe, b'b']);
    }

    /// A child killed by signal N surfaces as 128+N.
    #[cfg(unix)]
    #[test]
    fn a_signalled_child_surfaces_as_128_plus_n() {
        let exec = sh("kill -TERM $$", None);
        let outcome = run(&exec, |_, _| {}).expect("the child runs");
        assert_eq!(outcome.exit_code, 128 + 15);
    }

    #[test]
    fn redaction_replaces_every_occurrence() {
        let secrets = vec![b"sekret".to_vec()];
        assert_eq!(
            redact(b"a sekret and a sekret", &secrets),
            b"a [redacted] and a [redacted]".to_vec()
        );
        assert_eq!(redact(b"clean", &secrets), b"clean".to_vec());
    }
}
