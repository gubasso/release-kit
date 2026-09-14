//! release-kit: a canonical release workflow, carried whole by one binary.
//!
//! The library exists so the crate's own tests can link the modules; the
//! `rk` binary in `main.rs` is the product. `embedded` holds the embedded
//! sources and `distribution_roots` their one inventory, `cli` the
//! argument surface, `commands` the handlers, `projection` the one pure
//! candidate tree over the embedded sources, `landing` the parameters and
//! the direct writes into a target, `stage` the disposable candidate
//! stage over the projection, `self_depend` the consumer pin, `depend`
//! the dependency matrix, `skills` the user-scope skill install, `digest`
//! the one hash type, and `error` the one exit-code matrix.

pub mod applog;
pub mod assess;
pub mod atomic;
pub mod branches;
pub mod cli;
pub mod commands;
pub mod config;
pub mod depend;
pub mod detect;
pub mod diagnostic;
pub mod digest;
pub mod distribution_roots;
pub mod embedded;
pub mod error;
pub mod events;
pub(crate) mod held;
pub mod issue;
pub mod landing;
pub mod maintenance;
pub mod output;
pub mod probes;
pub mod projection;
pub mod registry;
pub mod self_depend;
pub mod setup;
pub mod skills;
pub mod stage;
pub mod worktree;
