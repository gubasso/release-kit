//! `rk forge`: read the per-forge documents, the siblings of the bindings.

use crate::cli::read::ReadArgs;
use crate::commands::walk;
use crate::embedded;
use crate::error::RkError;
use crate::output::Output;

/// Print one forge document byte-identically, or list the forges.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown forge and
/// [`RkError::Usage`] when neither a name nor `--list` is given.
pub fn run(args: &ReadArgs) -> Result<(), RkError> {
    let out = Output::human();
    let entries = walk(&embedded::FORGES);
    if args.list {
        for (path, _) in &entries {
            out.result_line(path.trim_end_matches(".md").to_ascii_lowercase());
        }
        return Ok(());
    }
    let Some(name) = args.name.as_deref() else {
        return Err(RkError::Usage(
            "name a forge, or pass --list to see them".into(),
        ));
    };
    let wanted = name.to_ascii_lowercase();
    let wanted = wanted.trim_end_matches(".md");
    entries
        .iter()
        .find(|(path, _)| path.trim_end_matches(".md").eq_ignore_ascii_case(wanted))
        .map_or_else(
            || {
                Err(RkError::NotFound {
                    kind: "forge",
                    name: name.to_owned(),
                })
            },
            |(_, contents)| {
                out.result_raw(&String::from_utf8_lossy(contents));
                Ok(())
            },
        )
}
