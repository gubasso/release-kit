//! One handler per subcommand. Handlers hold the behavior; `main` only
//! dispatches, and `cli` only declares the argument surface.

pub mod completions;
pub mod init;
pub mod license;
pub mod payload;
pub mod read;
pub mod skill;
pub mod versions;

pub(crate) use crate::embedded::walk;
