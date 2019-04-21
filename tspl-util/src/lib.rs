//! Utility functions for all trainsplorer crates.
//!
//! Currently contains:
//!
//! - random macros
//! - logging
//! - config

use config as cfg;
use serde::de::DeserializeOwned;
use log::*;

#[macro_export]
macro_rules! crate_name {
    () => {module_path!().split("::").next().unwrap()}
}

#[macro_export]
macro_rules! impl_from_for_error {
    ($error:ident, $($orig:ident => $var:ident),*) => {
        $(
            impl From<$orig> for $error {
                fn from(err: $orig) -> $error {
                    $error::$var(err)
                }
            }
         )*
    }
}

/// Extension trait for populating crate configuration structs.
///
/// If a struct used for config implements `Deserialize`, this
/// trait can be used to populate it with values from `[crate name].toml`
/// in the current directory, and from `TSPL_*` environment variables.
pub trait ConfigExt: DeserializeOwned {
    fn crate_name() -> &'static str;
    fn load() -> Result<Self, failure::Error> {
        let cn = Self::crate_name();
        info!("Loading trainsplorer config for crate {}", cn);
        let mut settings = cfg::Config::default();
        if let Err(e) = settings.merge(cfg::File::with_name(cn)) {
            warn!("Error loading config from file: {}", e);
            settings = cfg::Config::default();
        }
        let mut s2 = settings.clone();
        if let Err(e) = s2.merge(cfg::Environment::with_prefix("TSPL")) {
            warn!("Error loading config from env: {}", e);
        }
        else {
            settings = s2;
        }
        let ret = settings.try_into()?;
        Ok(ret)
    }
}

/// Initialize logging.
/// 
/// In the future, this will be slightly more clever.
pub fn setup_logging() -> Result<(), failure::Error> {
    fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!("[{} {}] {}",
                                    record.target(),
                                    record.level(),
                                    msg))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
