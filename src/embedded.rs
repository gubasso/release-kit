//! The compile-time payload: everything the binary serves or lands.
//!
//! `include_dir!` embeds each authored root at compile time, so the binary
//! and the canon it carries cannot drift; `build.rs` names the same roots
//! so any change under them rebuilds the crate.

use include_dir::{Dir, include_dir};

/// The technology-agnostic method chapters.
pub static METHOD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/method");

/// The per-technology bindings.
pub static BINDINGS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/bindings");

/// The deterministic files `rk init` lands, one subtree per technology,
/// laid out exactly as they land in a target repository.
pub static SNIPPETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/snippets");

/// The agent skills, one directory per skill.
pub static SKILLS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/skills");

/// The pinned-tool registry.
pub static VERSIONS: &str = include_str!("../versions.toml");

/// The root license statement naming both halves.
pub static LICENSE: &str = include_str!("../LICENSE");

/// The MIT text covering the distribution.
pub static LICENSE_MIT: &str = include_str!("../LICENSE-MIT");

/// The CC BY 4.0 text covering the method.
pub static LICENSE_CC_BY: &str = include_str!("../LICENSE-CC-BY-4.0");

/// The sentinel marker a landed file may carry; `rk init --apply` reports
/// every line holding one so nothing lands half-configured silently.
pub const SENTINEL: &str = "TODO(release-kit)";
