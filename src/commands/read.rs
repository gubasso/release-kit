//! The read-only payload commands: method, binding, snippet.

use include_dir::Dir;

use crate::cli::read::ReadArgs;
use crate::commands::walk;
use crate::embedded;
use crate::error::RkError;
use crate::output::Output;

/// Print a method chapter, or list the chapters.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown chapter and
/// [`RkError::Usage`] when neither a name nor `--list` is given.
pub fn method(args: &ReadArgs) -> Result<(), RkError> {
    flat(&embedded::METHOD, "chapter", args)
}

/// Print a technology binding, or list the bindings.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown binding and
/// [`RkError::Usage`] when neither a name nor `--list` is given.
pub fn binding(args: &ReadArgs) -> Result<(), RkError> {
    flat(&embedded::BINDINGS, "binding", args)
}

/// Print one landable file, or list them all as `<tech>/<path>`.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown path and
/// [`RkError::Usage`] when neither a name nor `--list` is given.
pub fn snippet(args: &ReadArgs) -> Result<(), RkError> {
    let out = Output::human();
    let entries = walk(&embedded::SNIPPETS);
    if args.list {
        for (path, _) in &entries {
            out.result_line(path);
        }
        return Ok(());
    }
    let Some(name) = args.name.as_deref() else {
        return Err(RkError::Usage(
            "name a snippet path, or pass --list to see them".into(),
        ));
    };
    entries.iter().find(|(path, _)| path == name).map_or_else(
        || {
            Err(RkError::NotFound {
                kind: "snippet",
                name: name.to_owned(),
            })
        },
        |(_, contents)| {
            print_bytes(out, contents);
            Ok(())
        },
    )
}

/// Resolve a name against the markdown files of one flat embedded root.
///
/// A chapter file `NN-name.md` answers to `name`, `NN-name`, and
/// `NN-name.md`; `README.md` answers to `readme`.
fn flat(dir: &Dir<'static>, kind: &'static str, args: &ReadArgs) -> Result<(), RkError> {
    let out = Output::human();
    let entries = walk(dir);
    if args.list {
        for (path, _) in &entries {
            out.result_line(short_key(path));
        }
        return Ok(());
    }
    let Some(name) = args.name.as_deref() else {
        return Err(RkError::Usage(format!(
            "name a {kind}, or pass --list to see them"
        )));
    };
    let wanted = name.to_ascii_lowercase();
    let wanted = wanted.trim_end_matches(".md");
    for (path, contents) in &entries {
        let stem = path.trim_end_matches(".md");
        if stem.eq_ignore_ascii_case(wanted) || short_key(path) == wanted {
            print_bytes(out, contents);
            return Ok(());
        }
    }
    Err(RkError::NotFound {
        kind,
        name: name.to_owned(),
    })
}

/// The listing key for a payload file: the stem, lowercased, with a
/// leading `NN-` chapter prefix stripped.
fn short_key(path: &str) -> String {
    let stem = path.trim_end_matches(".md");
    let stem = stem
        .split_once('-')
        .filter(|(prefix, _)| prefix.chars().all(|c| c.is_ascii_digit()))
        .map_or(stem, |(_, rest)| rest);
    stem.to_ascii_lowercase()
}

/// Print payload bytes as-is; every authored payload file is UTF-8.
fn print_bytes(out: Output, contents: &[u8]) {
    out.result_raw(&String::from_utf8_lossy(contents));
}
