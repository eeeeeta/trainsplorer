//! Stores live and historic train running information, as well as handling activations.

pub mod errors;

use log::*;
use errors::Result;

fn main() -> Result<()> {
    tspl_util::setup_logging()?;
    info!("tspl-zugfuhrer, but not yet");
    Ok(())
}
