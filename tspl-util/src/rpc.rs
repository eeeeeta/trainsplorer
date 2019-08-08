//! Handling remote procedure calls (RPC) to other microservices.

use reqwest::Client;
use reqwest::Error as ReqwestError;
use reqwest::header::HeaderMap;
pub use reqwest::Method;
use failure_derive::Fail;
use std::fmt::Display;
use serde::de::DeserializeOwned;
use log::*;

use crate::impl_from_for_error;

/// An error encountered after an RPC call.
#[derive(Debug, Fail)]
pub enum RpcError {
    /// The remote entity was not found.
    #[fail(display = "not found (remote)")]
    RemoteNotFound,
    /// The remote service was unavailable.
    #[fail(display = "remote service unavailable")]
    RemoteServiceUnavailable,
    /// The remote service returned an error.
    #[fail(display = "{} error (code {}): {}", service, code, error)]
    RemoteError {
        /// Name of the microservice responsible.
        service: &'static str,
        /// The HTTP status code returned.
        code: u16,
        /// The error text.
        error: String
    },
    /// reqwest error.
    #[fail(display = "reqwest: {}", _0)]
    Reqwest(ReqwestError)
}
impl_from_for_error!(RpcError,
                     ReqwestError => Reqwest);

impl RpcError {
    pub fn status_code(&self) -> u16 {
        use self::RpcError::*;
        match *self {
            RemoteNotFound => 404,
            RemoteServiceUnavailable => 503,
            RemoteError { .. } => 502,
            _ => 500
        }
    }
}

#[derive(Clone)]
pub struct MicroserviceRpc {
    pub base_url: String,
    pub user_agent: String,
    pub name: &'static str,
    pub cli: Client
}
impl MicroserviceRpc {
    pub fn new(ua: String, name: &'static str, base_url: String) -> Self {
        let cli = Client::new();
        Self {
            user_agent: ua,
            name, base_url, cli
        }
    }
    pub fn req_with_headers<T, U>(&self, meth: Method, url: T, hdrs: HeaderMap) -> Result<U, RpcError> where T: Display, U: DeserializeOwned {
        let url = format!("{}{}", self.base_url, url);
        debug!("RPC ({}): {} {}", self.name, meth, url);
        let mut resp = self.cli.request(meth, &url)
            .header(reqwest::header::USER_AGENT, &self.user_agent as &str)
            .headers(hdrs)
            .send()?;
        let status = resp.status();
        debug!("RPC ({}): response code {}", self.name, status.as_u16());
        match status.as_u16() {
            404 => Err(RpcError::RemoteNotFound)?,
            503 => Err(RpcError::RemoteServiceUnavailable)?,
            _ => {}
        }
        if !status.is_success() {
            let text = resp.text()?;
            warn!("RPC ({}): request failed ({}): {}", self.name, status.as_u16(), text);
            Err(RpcError::RemoteError {
                service: self.name,
                code: status.as_u16(),
                error: text
            })?
        }
        let ret: U = resp.json()?;
        Ok(ret)
    }
    pub fn req<T, U>(&self, meth: Method, url: T) -> Result<U, RpcError> where T: Display, U: DeserializeOwned {
        let hdrs = HeaderMap::new();
        self.req_with_headers(meth, url, hdrs)
    }
}
