//! Bog-standard configuration.

use serde_derive::Deserialize;
use tspl_util::{ConfigExt, crate_name};

/// `tspl-nrod` configuration.
#[derive(Deserialize, Debug)]
pub struct Config {
    /// URL of a running tspl-zugfuhrer instance.
    pub service_zugfuhrer: String,
    /// NROD/Darwin username.
    pub username: String,
    /// NROD/Darwin password.
    pub password: String,
    /// Number of worker threads to use.
    pub n_threads: u32,
    /// NROD/Darwin STOMP hostname.
    #[serde(default)]
    pub stomp_host: Option<String>,
    /// NROD/Darwin STOMP port.
    #[serde(default)]
    pub stomp_port: Option<u16>,
    /// Connect to Darwin instead of NROD.
    #[serde(default)]
    pub use_darwin: bool,
    /// Darwin queue for updates.
    #[serde(default)]
    pub darwin_queue_updates: Option<String>
}

impl ConfigExt for Config {
    fn crate_name() -> &'static str {
        crate_name!()
    }
}
