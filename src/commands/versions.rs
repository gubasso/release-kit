//! `rk versions`: print the pinned-tool registry.

use crate::embedded;
use crate::error::RkError;

/// Print the registry exactly as authored.
///
/// # Errors
///
/// Never fails; the signature matches the dispatch table.
pub fn run() -> Result<(), RkError> {
    print!("{}", embedded::VERSIONS);
    Ok(())
}
