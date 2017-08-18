extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate chrono;
#[cfg(test)] extern crate serde_json;

mod fns;
pub mod schedule;
pub mod vstp;
pub mod movements;
pub mod cif;
#[cfg(test)]
mod tests;

