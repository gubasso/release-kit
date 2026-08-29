//! release-kit: a canonical release workflow, carried whole by one binary.
//!
//! The library exists so the crate's own tests can link the modules; the
//! `rk` binary in `main.rs` is the product. `embedded` holds the payload
//! and `payload_roots` its one inventory, `cli` the argument surface,
//! `commands` the handlers, `skills` the user-scope skill install,
//! `digest` the one hash type, and `error` the one exit-code matrix.

pub mod cli;
pub mod commands;
pub mod digest;
pub mod embedded;
pub mod error;
pub mod payload_roots;
pub mod skills;
