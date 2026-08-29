//! One handler per subcommand. Handlers hold the behavior; `main` only
//! dispatches, and `cli` only declares the argument surface.

pub mod completions;
pub mod doctor;
pub mod forge;
pub mod guide;
pub mod init;
pub mod license;
pub mod payload;
pub mod read;
pub mod runs;
pub mod setup;
pub mod skill;
pub mod usage;
pub mod versions;

pub(crate) use crate::embedded::walk;
