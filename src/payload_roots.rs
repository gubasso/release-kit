// The payload inventory: every authored root the binary carries.
//
// One declaration, three readers. `embedded` must embed each root and a
// unit test holds it to this list; `build.rs` `include!`s this file to
// emit change tracking per root; and a packaging test asserts
// `cargo package --list` carries every root, so an `exclude` entry in
// `Cargo.toml` can never silently drop one from the published crate.
//
// `build.rs` pastes this file verbatim, so it holds items and plain
// comments only — no `use`, no inner doc comments, no other modules.

/// Every authored root the binary carries, in one place.
pub const PAYLOAD_ROOTS: [&str; 10] = [
    "method",
    "bindings",
    "runbooks",
    "forges",
    "snippets",
    "blocks",
    "setup",
    "skills",
    "skill-shared",
    "versions.toml",
];
