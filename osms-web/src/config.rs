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
        settings
            .merge(cfg::File::with_name("osms-web"))?
            .merge(cfg::Environment::with_prefix("OSMS"))?;
        let ret = settings.try_into()?;
        Ok(ret)
    }
}
