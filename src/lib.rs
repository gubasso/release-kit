//! release-kit: a canonical release workflow, carried whole by one binary.
//!
//! The library exists so the crate's own tests can link the modules; the
//! `rk` binary in `main.rs` is the product. `embedded` holds the payload,
//! `cli` the argument surface, `commands` the handlers, `skills` the
//! user-scope skill install, and `error` the one exit-code matrix.

pub mod cli;
pub mod commands;
pub mod embedded;
pub mod error;
pub mod skills;
