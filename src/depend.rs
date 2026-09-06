//! `rk depend`: another project as a dependency of a target, from how
//! the source distributes itself and how the target manages its tools.
//!
//! Two offline observations meet in a matrix. `source` reads the
//! dependency's checkout for its distribution channels, `target` reads
//! the project for its tool managers, `version` resolves the pin,
//! `matrix` pairs a manager with a channel and says whether the pair is a
//! fragment, a native command, or a hand edit, `fragments` renders the
//! authored texts, and `nix` holds the lexical scanners both sides share.
//! Nothing here edits a file the target owns.

pub mod fragments;
pub mod matrix;
pub mod nix;
pub mod source;
pub mod target;
pub mod version;

use camino::{Utf8Path, Utf8PathBuf};

pub use crate::cli::depend::{Channel, Kind, Manager};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// A directory argument as a canonical path, or the missing refusal
/// naming its role.
///
/// # Errors
///
/// Returns [`RkError::Missing`] where the path is not a directory.
pub fn canonical_dir(path: &Utf8Path, role: &'static str) -> Result<Utf8PathBuf, RkError> {
    if !path.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("{role} {path} is not a directory"),
            )
            .expected(format!("an existing {role} directory to read")),
        ));
    }
    Ok(path.canonicalize_utf8()?)
}

/// Refuse a `--source` that is a URL before the disk is read: the clone
/// is the operator's step, and a fetch is never implied by a read verb.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for a value carrying a scheme or the
/// `git@` form.
pub fn reject_url(raw: &Utf8Path) -> Result<(), RkError> {
    let text = raw.as_str();
    if text.contains("://") || text.starts_with("git@") {
        return Err(RkError::Usage(
            "--source takes a local checkout; clone the URL first, then pass the directory".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::{canonical_dir, reject_url};
    use crate::error::RkError;

    #[test]
    fn a_url_is_refused_before_the_disk_is_read() {
        for raw in [
            "https://github.com/owner/repo",
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo",
        ] {
            assert!(
                matches!(reject_url(Utf8Path::new(raw)), Err(RkError::Usage(_))),
                "{raw} is refused"
            );
        }
        assert!(reject_url(Utf8Path::new("../repo")).is_ok());
    }

    #[test]
    fn a_missing_directory_is_a_missing_error() {
        let result = canonical_dir(Utf8Path::new("/nonexistent/depend/source"), "source");
        assert!(matches!(result, Err(RkError::Missing(_))));
        assert_eq!(result.err().map(|e| e.exit_code()), Some(66));
    }
}
