//! Utility functions for HTTP servers.

pub use rouille::{Request, Response};
use std::fmt::Display;
use std::time::Instant;
use std::sync::Arc;
use log::*;

#[macro_export]
macro_rules! extract_headers {
    ($from:ident, $err:expr, 
     $(let $var:ident: $ty:ty => $header_name:literal),*
     $(,opt $ovar:ident: $oty:ty => $oheader_name:literal)*
    ) => {
        $(
            let $var: $ty = $from.header(concat!("X-tspl-", $header_name))
                .ok_or($err)?
                .parse()
                .map_err(|_| $err)?;
        )*
        $(
            let $ovar: Option<$oty> = match $from.header(concat!("X-tspl-", $oheader_name)) {
                Some(h) => {
                    Some(h
                         .parse()
                         .map_err(|_| $err)?)
                },
                None => None
            };
        )*
    }
}
/// Trait for errors that have an associated status code.
pub trait StatusCode {
    /// Returns the status code associated with this error.
    fn status_code(&self) -> u16;
}

/// Trait for HTTP server objects that can process requests.
pub trait HttpServer: Sync + Send + 'static {
    type Error: StatusCode + Display;
    /// Handle the given HTTP request, returning either a HTTP response
    /// or an error.
    fn on_request(&self, req: &Request) -> Result<Response, Self::Error>;
    /// Helper method that actually returns a `Response` in all cases,
    /// calling `on_request` to do the actual work.
    ///
    /// Also provides handy logging.
    fn process_request(&self, req: &Request) -> Response {
        let start = Instant::now();
        let ret = self.on_request(req);
        let ret = match ret {
            Ok(r) => r,
            Err(e) => {
                let sc = e.status_code();
                warn!("Processing request failed ({}): {}", sc, e);
                Response::text(format!("error: {}\n", e))
                    .with_status_code(sc)
            }
        };
        let dur = start.elapsed();
        info!("{} {} \"{}\" - {} [{}.{:03}s]", req.remote_addr(), req.method(), req.raw_url(), ret.status_code, dur.as_secs(), dur.subsec_millis());
        ret
    }
}

impl<T> HttpServer for Arc<T> where T: HttpServer {
    type Error = T::Error;
    fn on_request(&self, req: &Request) -> Result<Response, T::Error> {
        use std::ops::Deref;

        self.deref().on_request(req)
    }
}
/// Starts an HTTP server, listening on the provided address.
pub fn start_server<H: HttpServer>(listen_url: &str, srv: H) -> ! {
    info!("Starting HTTP server on {}", listen_url);
    rouille::start_server(listen_url, move |req| {
        srv.process_request(req)
    })
}
