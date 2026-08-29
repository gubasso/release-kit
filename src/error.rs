//! Crate-level error type and exit-code mapping.
//!
//! [`RkError`] aggregates errors from every layer via `#[from]`, and
//! [`RkError::exit_code`] maps each variant to its process exit code. No
//! other module decides exit codes. Exit `1` stays reserved for one meaning
//! — a check ran and found violations — while the BSD sysexits range covers
//! the tool failing to do its job at all. Beside the code, every failure
//! carries a [`Reason`] from the closed vocabulary in [`crate::diagnostic`]:
//! the code is the category, the reason the instance.

use thiserror::Error;

use crate::diagnostic::{Diagnostic, Reason};

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

    /// A refusal with its parts named, carrying its own reason and the
    /// hints the boundary renders. New refusals use this; [`Self::Refused`]
    /// remains for the paths not yet carrying structured hints.
    #[error("{}", .0.message)]
    Refusal(Box<Diagnostic>),

    /// A named input is absent — a target that is not a repository, or
    /// detection that finds no remote — mapping to the sysexits no-input
    /// code rather than the refusal code.
    #[error("{}", .0.message)]
    Missing(Box<Diagnostic>),

    /// A check ran and found violations: the one sanctioned bare exit 1,
    /// after the per-item report has already been rendered.
    #[error("{}", .0.message)]
    CheckFailed(Box<Diagnostic>),

    /// A child process ran and failed for a reason this binary cannot
    /// classify further; the child's own stderr travels with it.
    #[error("{}", .0.message)]
    Subprocess(Box<Diagnostic>),

    /// Filesystem failure, classified by its I/O kind.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Escape hatch for ad-hoc contexts at the binary boundary.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl RkError {
    /// A [`Self::Refusal`] from a built diagnostic.
    #[must_use]
    pub fn refusal(diagnostic: Diagnostic) -> Self {
        Self::Refusal(Box::new(diagnostic))
    }

    /// A [`Self::Missing`] from a built diagnostic.
    #[must_use]
    pub fn missing(diagnostic: Diagnostic) -> Self {
        Self::Missing(Box::new(diagnostic))
    }

    /// A [`Self::CheckFailed`] from a built diagnostic.
    #[must_use]
    pub fn check_failed(diagnostic: Diagnostic) -> Self {
        Self::CheckFailed(Box::new(diagnostic))
    }

    /// A [`Self::Subprocess`] from a built diagnostic.
    #[must_use]
    pub fn subprocess(diagnostic: Diagnostic) -> Self {
        Self::Subprocess(Box::new(diagnostic))
    }

    /// Map to the process exit code.
    ///
    /// `64..=78` follow BSD `sysexits(3)` and mean the tool itself could
    /// not do its job.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 64,
            Self::NotFound { .. } | Self::Missing(_) => 66,
            Self::CheckFailed(_) => 1,
            Self::Refused(_) | Self::Refusal(_) => 73,
            Self::Io(e) if e.kind() == std::io::ErrorKind::NotFound => 66,
            Self::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => 77,
            Self::Io(_) => 74,
            Self::Subprocess(_) | Self::Other(_) => 70,
        }
    }

    /// The reason beside the code: a [`Self::Refusal`] carries its own,
    /// and every other variant maps to its honest coarse entry.
    #[must_use]
    pub fn reason(&self) -> Reason {
        match self {
            Self::Usage(_) | Self::NotFound { .. } => Reason::Usage,
            Self::Refused(_) => Reason::StateDrift,
            Self::Refusal(diagnostic)
            | Self::Missing(diagnostic)
            | Self::CheckFailed(diagnostic)
            | Self::Subprocess(diagnostic) => diagnostic.reason,
            Self::Io(_) => Reason::Io,
            Self::Other(_) => Reason::Internal,
        }
    }

    /// The typed form of this failure, for the JSON rendering.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Refusal(diagnostic)
            | Self::Missing(diagnostic)
            | Self::CheckFailed(diagnostic)
            | Self::Subprocess(diagnostic) => (**diagnostic).clone(),
            _ => Diagnostic::new(self.reason(), self.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RkError;
    use crate::diagnostic::{Diagnostic, Reason};

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
                RkError::refusal(Diagnostic::new(Reason::TargetNotFound, "no target")),
                73,
            ),
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

    /// Beside the code, every variant answers with a reason, and a refusal
    /// keeps the one it was built with.
    #[test]
    fn every_error_carries_a_reason() {
        assert_eq!(RkError::Usage("bad".into()).reason(), Reason::Usage);
        assert_eq!(
            RkError::Refused("left unchanged".into()).reason(),
            Reason::StateDrift
        );
        assert_eq!(
            RkError::refusal(Diagnostic::new(Reason::JournalUnavailable, "no journal")).reason(),
            Reason::JournalUnavailable
        );
        assert_eq!(
            RkError::Other(anyhow::anyhow!("unclassified")).reason(),
            Reason::Internal
        );
    }
}
