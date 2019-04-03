//! Configuration management.
use cfg;
use super::Result;

#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    pub listen: String
}
impl Config {
    pub fn load() -> Result<Self> {
        let mut settings = cfg::Config::default();
        if let Err(e) = settings.merge(cfg::File::with_name("osms-web")) {
            eprintln!("Error loading config from file: {}", e);
        }
        if let Err(e) = settings.merge(cfg::Environment::with_prefix("OSMS")) {
            eprintln!("Error loading config from env: {}", e);
        }
        let ret = settings.try_into()?;
        Ok(ret)
    }
}
