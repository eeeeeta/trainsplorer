//! The tspl-sqlite crate provides a common set of functions
//! for initializing, migrating and managing SQLite databases.
//!
//! It also includes some traits used to make common DB operations
//! (such as SELECT) easier.

pub mod errors;
pub mod traits;
pub mod migrations;
