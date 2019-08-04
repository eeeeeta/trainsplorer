//! Standard configuration module.

use serde_derive::Deserialize;
use tspl_util::{ConfigExt, crate_name};

/// `tspl-zugfuhrer` configuration.
#[derive(Deserialize, Debug)]
pub struct Config {
    /// Path to SQLite database.
    pub database_path: String,
}

impl ConfigExt for Config {
    fn crate_name() -> &'static str {
        crate_name!()
    }
}
