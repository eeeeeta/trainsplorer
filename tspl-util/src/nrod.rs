//! Downloading gzipped files from NROD.

use reqwest::{Client, Response};
use reqwest::Error as ReqwestError;
use std::io::BufReader;
use flate2::bufread::GzDecoder;
use failure_derive::Fail;
use crate::impl_from_for_error;
use log::*;

#[derive(Fail, Debug)]
pub enum NrodDownloadError {
    /// reqwest error.
    #[fail(display = "reqwest: {}", _0)]
    Reqwest(ReqwestError),
    /// Unexpected response code.
    #[fail(display = "unexpected status code {} from NROD", _0)]
    StatusCode(u16)
}
impl_from_for_error!(NrodDownloadError,
                     ReqwestError => Reqwest);

pub struct NrodDownloader {
    username: String,
    password: String,
    base_url: String,
    cli: Client,
}
// FIXME(perf): Yes, two layers of buffering.
//
// - The first layer of buffering is to avoid copious syscalls
//   when reading from the network. Not sure whether it's really needed.
// - The second layer of buffering is required by `apply_schedule_records`,
//   which needs to call read_line().
//
// I really don't know how performant this is. My guess is "not very"???
pub type ResponseReader = BufReader<GzDecoder<BufReader<Response>>>;

impl NrodDownloader {
    pub fn new(username: String, password: String, base_url: Option<String>) -> Self {
        let cli = reqwest::Client::new();
        Self {
            username,
            password,
            base_url: base_url
                .unwrap_or_else(|| "https://datafeeds.networkrail.co.uk".into()),
            cli
        }
    }
    pub fn download(&mut self, url: &str) -> Result<ResponseReader, NrodDownloadError> {
        debug!("Requesting from NROD: {}", url);
        let url = format!("{}{}", self.base_url, url);
        let resp = self.cli.get(&url as &str)
            .basic_auth(&self.username, Some(&self.password))
            .send()?;
        let st = resp.status();
        if !st.is_success() {
            Err(NrodDownloadError::StatusCode(st.as_u16()))?
        }
        let resp = BufReader::new(GzDecoder::new(BufReader::new(resp)));
        Ok(resp)
    }
}
