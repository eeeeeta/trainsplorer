//! Bog-standard configuration.

use serde_derive::Deserialize;
use tspl_util::{ConfigExt, crate_name};

/// `tspl-nrod` configuration.
#[derive(Deserialize, Debug)]
pub struct Config {
    /// URL of a running tspl-zugfuhrer instance.
    pub service_zugfuhrer: String,
    /// NROD username.
    pub username: String,
    /// NROD password.
    pub password: String,
    /// Number of worker threads to use.
    pub n_threads: u32,
    /// NROD STOMP hostname.
    #[serde(default)]
    pub stomp_host: Option<String>,
    /// NROD STOMP port.
    #[serde(default)]
    pub stomp_port: Option<u16>
}

impl ConfigExt for Config {
    fn crate_name() -> &'static str {
        crate_name!()
    }
}
