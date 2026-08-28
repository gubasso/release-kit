//! One handler per subcommand. Handlers hold the behavior; `main` only
//! dispatches, and `cli` only declares the argument surface.

pub mod completions;
pub mod init;
pub mod license;
pub mod read;
pub mod skill;
pub mod versions;

use include_dir::Dir;

/// Collect every file under `dir`, depth-first, as `(path, contents)` with
/// the path relative to the embedded root.
pub(crate) fn walk<'a>(dir: &Dir<'a>) -> Vec<(String, &'a [u8])> {
    let mut out = Vec::new();
    for file in dir.files() {
        out.push((file.path().to_string_lossy().into_owned(), file.contents()));
    }
    for sub in dir.dirs() {
        out.extend(walk(sub));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
