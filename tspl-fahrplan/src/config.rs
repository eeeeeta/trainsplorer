/// Configuration!

use serde_derive::Deserialize;
use tspl_util::{ConfigExt, crate_name};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bucket_name: String,
    pub service_account_key_path: String,
    #[serde(default)]
    pub object_name: Option<String>,
    pub username: String,
    pub password: String,
    pub listen_url: String,
    #[serde(default)]
    pub gcs_check_secs: Option<u32>,
}

impl ConfigExt for Config {
    fn crate_name() -> &'static str {
        crate_name!()
    }
}
