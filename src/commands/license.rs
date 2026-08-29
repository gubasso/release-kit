//! `rk license`: print the terms the binary carries.

use crate::embedded;
use crate::error::RkError;
use crate::output::Output;

/// Print the root statement, then both license texts.
///
/// # Errors
///
/// Never fails; the signature matches the dispatch table.
pub fn run() -> Result<(), RkError> {
    let out = Output::human();
    out.result_raw(embedded::LICENSE);
    out.result_line("");
    out.result_line("--- LICENSE-MIT ---");
    out.result_line("");
    out.result_raw(embedded::LICENSE_MIT);
    out.result_line("");
    out.result_line("--- LICENSE-CC-BY-4.0 ---");
    out.result_line("");
    out.result_raw(embedded::LICENSE_CC_BY);
    Ok(())
}
