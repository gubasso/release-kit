//! Crate-level error type and exit-code mapping.
//!
//! [`RkError`] aggregates errors from every layer via `#[from]`, and
//! [`RkError::exit_code`] maps each variant to its process exit code. No
//! other module decides exit codes. Exit `1` stays reserved for one meaning
//! — a check ran and found violations — while the BSD sysexits range covers
//! the tool failing to do its job at all.

use thiserror::Error;

/// Every failure the binary can exit with.
#[derive(Debug, Error)]
pub enum RkError {
    /// Semantically invalid arguments clap cannot reject on shape alone.
    #[error("usage: {0}")]
    Usage(String),

    /// A named payload entry does not exist; the caller can list the
    /// valid names with `--list`.
    #[error("no {kind} named '{name}'; run with --list to see the valid names")]
    NotFound {
        /// What class of entry was asked for: chapter, binding, snippet,
        /// or skill.
        kind: &'static str,
        /// The name that resolved to nothing.
        name: String,
    },

    /// The command refused to touch the target as found, and the target
    /// was left unchanged.
    #[error("{0}")]
    Refused(String),

    /// Filesystem failure, classified by its I/O kind.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Escape hatch for ad-hoc contexts at the binary boundary.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl RkError {
    /// Map to the process exit code.
    ///
    /// `64..=78` follow BSD `sysexits(3)` and mean the tool itself could
    /// not do its job.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 64,
            Self::NotFound { .. } => 66,
            Self::Refused(_) => 73,
            Self::Io(e) if e.kind() == std::io::ErrorKind::NotFound => 66,
            Self::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => 77,
            Self::Io(_) => 74,
            Self::Other(_) => 70,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RkError;

    #[test]
    fn exit_code_matrix() {
        let cases: Vec<(RkError, u8)> = vec![
            (RkError::Usage("bad".into()), 64),
            (
                RkError::NotFound {
                    kind: "chapter",
                    name: "nope".into(),
                },
                66,
            ),
            (RkError::Refused("left unchanged".into()), 73),
            (
                RkError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone")),
                66,
            ),
            (
                RkError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "denied",
                )),
                77,
            ),
            (RkError::Io(std::io::Error::other("disk fell over")), 74),
            (RkError::Other(anyhow::anyhow!("unclassified")), 70),
        ];
        for (err, code) in cases {
            assert_eq!(err.exit_code(), code, "wrong exit code for {err:?}");
        }
    }
}
