//! `rk usage`: the whole command tree in one call.
//!
//! Generated from the clap definitions, never hand-maintained, so an
//! agent loads the surface once instead of walking `--help` per
//! subcommand and the dump cannot describe a CLI it no longer matches.

use clap::CommandFactory;

use crate::cli::Cli;
use crate::error::RkError;
use crate::output::Output;

/// Print every verb, flag, default, and one example each.
///
/// # Errors
///
/// Never fails; the signature matches the dispatch table.
pub fn run() -> Result<(), RkError> {
    let out = Output::human();
    let root = Cli::command();
    out.result_line(format!(
        "rk {} — {}",
        env!("CARGO_PKG_VERSION"),
        root.get_about()
            .map(ToString::to_string)
            .unwrap_or_default()
    ));
    for sub in root.get_subcommands() {
        describe(out, sub, "rk");
    }
    out.next(&[
        "rk doctor reports whether this host is ready".to_owned(),
        "rk method --list starts the reading path".to_owned(),
    ]);
    Ok(())
}

/// Print one command's block, then recurse into its subcommands.
fn describe(out: Output, cmd: &clap::Command, prefix: &str) {
    let path = format!("{prefix} {}", cmd.get_name());
    if cmd.has_subcommands() {
        out.result_line(String::new());
        out.result_line(format!(
            "{path} — {}",
            cmd.get_about().map(ToString::to_string).unwrap_or_default()
        ));
        for sub in cmd.get_subcommands() {
            describe(out, sub, &path);
        }
        return;
    }
    out.result_line(String::new());
    out.result_line(format!(
        "{path} — {}",
        cmd.get_about().map(ToString::to_string).unwrap_or_default()
    ));
    out.result_line(format!("  example: {}", example(cmd, &path)));
    for arg in cmd.get_arguments() {
        if matches!(arg.get_id().as_str(), "help" | "version") {
            continue;
        }
        out.result_line(format!("  {}", describe_arg(arg)));
    }
}

/// One pasteable example: the command path plus every required argument
/// with a placeholder value.
fn example(cmd: &clap::Command, path: &str) -> String {
    use std::fmt::Write as _;
    let mut example = path.to_owned();
    for arg in cmd.get_arguments() {
        if !arg.is_required_set() {
            continue;
        }
        let value = format!("<{}>", arg.get_id().as_str().to_ascii_uppercase());
        match arg.get_long() {
            Some(long) => {
                let _ = write!(example, " --{long} {value}");
            }
            None => {
                let _ = write!(example, " {value}");
            }
        }
    }
    example
}

/// One argument line: the form, whether it is required, its help, and its
/// default where one exists.
fn describe_arg(arg: &clap::Arg) -> String {
    use std::fmt::Write as _;
    let takes_value = arg.get_num_args().is_none_or(|num| num.takes_values());
    let form = match (arg.get_long(), takes_value) {
        (Some(long), true) => format!("--{long} <{}>", arg.get_id().as_str().to_ascii_uppercase()),
        (Some(long), false) => format!("--{long}"),
        (None, _) => format!("[{}]", arg.get_id().as_str().to_ascii_uppercase()),
    };
    let mut line = form;
    if arg.is_required_set() {
        line.push_str("  (required)");
    }
    if let Some(help) = arg.get_help() {
        let _ = write!(line, "  {help}");
    }
    let defaults = arg.get_default_values();
    if !defaults.is_empty() {
        let rendered: Vec<String> = defaults
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        let _ = write!(line, " (default: {})", rendered.join(", "));
    }
    line
}
