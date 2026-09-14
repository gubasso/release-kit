//! Build script: embedded-asset change tracking only.
//!
//! `include_dir!` embeds at compile time but cargo does not watch a
//! directory for new or deleted files on its own; naming each embedded
//! root here makes any change under them rebuild the crate. The roots come
//! from the one inventory in `src/distribution_roots.rs`, so this list
//! cannot drift from what `embedded` serves. The licenses and the changelog
//! are embedded beside that inventory and tracked here by name. No code
//! generation happens.

include!("src/distribution_roots.rs");

fn main() {
    for root in DISTRIBUTION_ROOTS {
        println!("cargo:rerun-if-changed={root}");
    }
    for beside in [
        "LICENSE",
        "LICENSE-MIT",
        "LICENSE-CC-BY-4.0",
        "CHANGELOG.md",
    ] {
        println!("cargo:rerun-if-changed={beside}");
    }
}
