extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate hyper;
extern crate hyper_native_tls;
#[macro_use] extern crate error_chain;
extern crate chrono;
extern crate percent_encoding;

pub mod errors {
    error_chain! {
        types {
            RailError, RailErrorKind, ResultExt, RailResult;
        }
        foreign_links {
            Hyper(::hyper::error::Error);
            Serde(::serde_json::Error);
            Io(::std::io::Error);
            Ssl(::hyper_native_tls::native_tls::Error);
        }
        errors {
            HttpCode(c: ::hyper::status::StatusCode) {
                display("HTTP error: {}", c.canonical_reason().unwrap_or("unknown"))
            }
            BadRequest {
                display("Bad request")
            }
        }
    }
}
pub mod types;
use types::*;
use errors::*;
use errors::RailErrorKind::*;
use hyper_native_tls::NativeTlsClient;
use hyper::net::HttpsConnector;
use hyper::client::{Response, RequestBuilder};
use std::io::prelude::*;
use hyper::method::Method;
use Method::*;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};

pub struct RailClient {
    hyper: hyper::Client,
    access_token: String,
    url: String
}
impl RailClient {
    pub fn new(access_token: &str, url: &str) -> RailResult<Self> {
        let ssl = NativeTlsClient::new()?;
        let conn = HttpsConnector::new(ssl);
        let client = hyper::Client::with_connector(conn);
        Ok(Self {
            hyper: client,
            access_token: access_token.into(),
            url: url.into()
        })
    }
    fn handle_errs(resp: &mut Response) -> RailResult<()> {
        if !resp.status.is_success() {
            let st = resp.status.clone();
            if let Ok(e) = serde_json::from_reader::<_, BadRequestReply>(resp) {
                if e.message == "An error has occured." {
                    bail!(BadRequest);
                }
            }
            else {
                bail!(HttpCode(st));
            }
        }
        Ok(())
    }
    pub fn station_board(&mut self, station: &str, rows: u32) -> RailResult<StationBoard> {
        let mut resp = self.req(Get, &format!("/all/{}/{}", station, rows))
            .send()?;
        let mut st = String::new();
        Self::handle_errs(&mut resp)?;
        resp.read_to_string(&mut st)?;
        println!("{}", st);
        let raw: StationBoardRaw = serde_json::from_str(&st)?;
        Ok(raw.into())
    }
    pub fn service(&mut self, id: &str) -> RailResult<ServiceItem> {
        let mut resp = self.req(Get, &format!("/service/{}", id))
            .send()?;
        Self::handle_errs(&mut resp)?;
        let raw: ServiceItemRaw = serde_json::from_reader(resp)?;
        Ok(raw.into())
    }
    pub fn req(&mut self, meth: Method, endpoint: &str) -> RequestBuilder {
        let qs = format!("accessToken={}", self.access_token);
        let endpoint = utf8_percent_encode(endpoint, DEFAULT_ENCODE_SET);
        let url = format!("{}{}?{}", self.url, endpoint, qs);
        println!("{}", url);
        self.hyper.request(meth, &url)
    }
}
