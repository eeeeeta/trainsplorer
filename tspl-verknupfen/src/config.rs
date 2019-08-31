//! Standard configuration module.

use serde_derive::Deserialize;
use tspl_util::{ConfigExt, crate_name};

/// `tspl-verknupfen` configuration.
#[derive(Deserialize, Debug)]
pub struct Config {
    /// Address to listen on.
    pub listen: String,
    /// URL of a running tspl-fahrplan instance.
    pub service_fahrplan: String,
    /// URL of a running tspl-zugfuhrer instance.
    pub service_zugfuhrer: String,
}

impl ConfigExt for Config {
    fn crate_name() -> &'static str {
        crate_name!()
    }
}
