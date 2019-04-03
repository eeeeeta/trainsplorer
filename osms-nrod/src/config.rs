use std::collections::HashMap;
#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    #[serde(default)]
    pub database_tls: bool,
    #[serde(default)]
    pub statsd_url: Option<String>,
    pub username: String,
    pub password: String,
    pub darwin_username: String,
    pub darwin_password: String,
    pub darwin_queue_name: String,
    pub n_threads: u32,
    #[serde(default)]
    pub log_level_general: Option<String>,
    #[serde(default)]
    pub log_level: HashMap<String, String>,
    #[serde(default)]
    pub nrod_url: Option<String>,
    #[serde(default)]
    pub nrod_port: Option<u16>,
    #[serde(default)]
    pub darwin_url: Option<String>,
    #[serde(default)]
    pub darwin_port: Option<u16>
}
impl Config {
    pub fn load() -> Result<Self, failure::Error> {
        let mut settings = cfg::Config::default();
        if let Err(e) = settings.merge(cfg::File::with_name("osms-nrod")) {
            eprintln!("Error loading config from file: {}", e);
        }
        if let Err(e) = settings.merge(cfg::Environment::with_prefix("OSMS")) {
            eprintln!("Error loading config from env: {}", e);
        }
        let ret = settings.try_into()?;
        Ok(ret)
    }
}
