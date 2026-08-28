//! `rk init`: land a technology's deterministic files into a target.
//!
//! Dry-run by default: without `--apply` the destinations are listed and
//! nothing is touched. Apply is all-or-nothing against conflicts: every
//! destination whose bytes differ from the payload is reported and the
//! whole landing is refused, so a target is never left half-written.

use std::fs;

use camino::Utf8Path;

use crate::cli::init::InitArgs;
use crate::commands::walk;
use crate::embedded;
use crate::error::RkError;

/// Land the files for `--tech` into `--target`.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for an unknown technology,
/// [`RkError::Refused`] when the target is missing or a destination
/// conflicts, and [`RkError::Io`] on filesystem failure.
pub fn run(args: &InitArgs) -> Result<(), RkError> {
    let tech_dir = embedded::SNIPPETS.get_dir(&args.tech).ok_or_else(|| {
        let known: Vec<String> = embedded::SNIPPETS
            .dirs()
            .map(|d| d.path().to_string_lossy().into_owned())
            .collect();
        RkError::Usage(format!(
            "unknown tech '{}'; the bindings are: {}",
            args.tech,
            known.join(", ")
        ))
    })?;

    if !args.target.is_dir() {
        return Err(RkError::Refused(format!(
            "target {} is not a directory; nothing was written",
            args.target
        )));
    }

    // Payload paths carry the `<tech>/` prefix; destinations do not.
    let files: Vec<(String, &[u8])> = walk(tech_dir)
        .into_iter()
        .map(|(path, contents)| {
            let rel = path
                .strip_prefix(&format!("{}/", args.tech))
                .map_or(path.as_str(), |r| r)
                .to_owned();
            (rel, contents)
        })
        .collect();

    if !args.apply {
        println!(
            "DRY RUN: rk init writes these files into {}; re-run with --apply",
            args.target
        );
        for (rel, _) in &files {
            println!("{rel}");
        }
        return Ok(());
    }

    // Every destination is read before anything writes, so an unreadable
    // path — a directory where a file should land, a permission failure —
    // surfaces here and the target is never left half-written.
    let mut conflicts: Vec<&str> = Vec::new();
    for (rel, contents) in &files {
        let dest = args.target.join(rel);
        match fs::read(&dest) {
            Ok(found) if found == *contents => {}
            Ok(_) => conflicts.push(rel.as_str()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if !conflicts.is_empty() {
        return Err(RkError::Refused(format!(
            "these files exist with different content, and nothing was written: {}",
            conflicts.join(", ")
        )));
    }

    for (rel, contents) in &files {
        let dest = args.target.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if dest.is_file() {
            println!("unchanged {rel}");
        } else {
            fs::write(&dest, contents)?;
            println!("wrote {rel}");
        }
    }

    report_sentinels(&args.target, &files);
    Ok(())
}

/// Print every sentinel line left in the landed files, so nothing stays
/// half-configured silently.
fn report_sentinels(target: &Utf8Path, files: &[(String, &[u8])]) {
    let mut found = false;
    for (rel, contents) in files {
        let text = String::from_utf8_lossy(contents);
        for (idx, line) in text.lines().enumerate() {
            if line.contains(embedded::SENTINEL) {
                if !found {
                    println!("fill these sentinels before the workflow runs:");
                    found = true;
                }
                println!("{}:{}: {}", target.join(rel), idx + 1, line.trim());
            }
        }
    }
    if !found {
        println!("no sentinels to fill");
    }
}
