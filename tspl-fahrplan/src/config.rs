/// Configuration!

use serde_derive::Deserialize;
use tspl_util::{ConfigExt, crate_name};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub database_path: String,
    pub username: String,
    pub password: String,
    pub listen_url: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub update_hour: Option<u32>,
    #[serde(default)]
    pub update_timeout_ms: Option<u32>,
    #[serde(default)]
    pub update_retries: Option<u32>
}

impl ConfigExt for Config {
    fn crate_name() -> &'static str {
        crate_name!()
    }
}
