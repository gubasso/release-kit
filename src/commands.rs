//! One handler per subcommand. Handlers hold the behavior; `main` only
//! dispatches, and `cli` only declares the argument surface.

pub mod adopt;
pub mod branches;
pub mod completions;
pub mod doctor;
pub mod forge;
pub mod guide;
pub mod init;
pub mod license;
pub mod message;
pub mod payload;
pub mod read;
pub mod runs;
pub mod setup;
pub mod skill;
pub mod status;
pub mod upgrade;
pub mod usage;
pub mod versions;
pub mod worktree;

pub(crate) use crate::embedded::walk;
