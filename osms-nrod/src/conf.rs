use std::collections::HashMap;
use envy;

#[derive(Deserialize, Debug)]
pub struct NrodConfig {
    pub database_url: String,
    #[serde(default)]
    pub statsd_url: Option<String>,
    pub username: String,
    pub password: String,
    pub n_threads: u32,
    #[serde(default)]
    pub log_level_general: Option<String>,
    #[serde(default)]
    pub log_level: HashMap<String, String>,
    #[serde(default)]
    pub nrod_url: Option<String>,
    #[serde(default)]
    pub nrod_port: Option<u16>
}
impl NrodConfig {
    pub fn make() -> Result<Self, ::failure::Error> {
        let ret = envy::from_env()?;
        Ok(ret)
    }
}

