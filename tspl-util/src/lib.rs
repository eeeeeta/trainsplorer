//! Utility functions for all trainsplorer crates.
//!
//! Currently contains:
//!
//! - random macros
//! - logging
//!
//! Will probably soon contain:
//!
//! - config

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
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
