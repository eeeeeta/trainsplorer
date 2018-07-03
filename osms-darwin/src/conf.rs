use envy;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct DarwinConfig {
//    pub database_url: String,
//    #[serde(default)]
//    pub statsd_url: Option<String>,
    pub username: String,
    pub password: String,
    pub queue_name: String,
//    pub n_threads: u32,
    #[serde(default)]
    pub log_level_general: Option<String>,
    #[serde(default)]
    pub log_level: HashMap<String, String>,
    #[serde(default)]
    pub darwin_url: Option<String>,
    #[serde(default)]
    pub darwin_port: Option<u16>,
}
impl DarwinConfig {
    pub fn make() -> Result<Self, ::failure::Error> {
        let ret = envy::from_env()?;
        Ok(ret)
    }
}

