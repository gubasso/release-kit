//! The executable half of `method/02-setup.md`.
//!
//! Rust owns everything except the forge call: step selection, ordering and
//! prerequisite refusal, preview versus apply policy, detection, environment
//! construction, exit-code classification, and the run journal. Each
//! embedded script owns one forge-CLI invocation and the check it prints.
//! Every mutating step follows one lifecycle — observe, compare, apply,
//! verify — and preview, apply, and `check` are three callers of the same
//! observe-and-verify code, with the mutating half unreachable from the
//! read-only paths.

pub mod context;
pub mod journal;
pub mod observe;
pub mod process;
pub mod secrets;
pub mod steps;
