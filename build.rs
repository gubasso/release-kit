//! Build script: embedded-asset change tracking only.
//!
//! `include_dir!` embeds at compile time but cargo does not watch a
//! directory for new or deleted files on its own; naming each embedded
//! root here makes any change under them rebuild the crate. The roots come
//! from the one payload inventory in `src/payload_roots.rs`, so this list
//! cannot drift from what `embedded` serves. No code generation happens.

include!("src/payload_roots.rs");

fn main() {
    for root in PAYLOAD_ROOTS {
        println!("cargo:rerun-if-changed={root}");
    }
    for license in ["LICENSE", "LICENSE-MIT", "LICENSE-CC-BY-4.0"] {
        println!("cargo:rerun-if-changed={license}");
    }
}
