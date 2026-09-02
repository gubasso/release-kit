//! release-kit: a canonical release workflow, carried whole by one binary.
//!
//! The library exists so the crate's own tests can link the modules; the
//! `rk` binary in `main.rs` is the product. `embedded` holds the payload
//! and `payload_roots` its one inventory, `cli` the argument surface,
//! `commands` the handlers, `skills` the user-scope skill install,
//! `digest` the one hash type, and `error` the one exit-code matrix.

pub mod applog;
pub mod atomic;
pub mod branches;
pub mod cli;
pub mod commands;
pub mod detect;
pub mod diagnostic;
pub mod digest;
pub mod embedded;
pub mod error;
pub mod events;
pub mod landing;
pub mod output;
pub mod payload_roots;
pub mod probes;
pub mod registry;
pub mod setup;
pub mod skills;
