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
            settings = cfg::Config::default();
        }
        let mut s2 = settings.clone();
        if let Err(e) = s2.merge(cfg::Environment::with_prefix("OSMS")) {
            eprintln!("Error loading config from env: {}", e);
        }
        else {
            settings = s2;
        }
        let ret = settings.try_into()?;
        Ok(ret)
    }
}
