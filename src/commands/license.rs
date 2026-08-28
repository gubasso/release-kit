//! `rk license`: print the terms the binary carries.

use crate::embedded;
use crate::error::RkError;

/// Print the root statement, then both license texts.
///
/// # Errors
///
/// Never fails; the signature matches the dispatch table.
pub fn run() -> Result<(), RkError> {
    print!("{}", embedded::LICENSE);
    println!();
    println!("--- LICENSE-MIT ---");
    println!();
    print!("{}", embedded::LICENSE_MIT);
    println!();
    println!("--- LICENSE-CC-BY-4.0 ---");
    println!();
    print!("{}", embedded::LICENSE_CC_BY);
    Ok(())
}
