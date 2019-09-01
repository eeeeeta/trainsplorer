//! Error handling, but probably a bit snazzier.

pub use failure::Error;
use failure_derive::Fail;
use tspl_util::impl_from_for_error;
use tspl_util::http::StatusCode;
use tspl_sqlite::errors::{SqlError, PoolError};
use handlebars::RenderError;
use reqwest::Error as ReqwestError;
use tspl_util::rpc::RpcError;
use handlebars::Handlebars;
use rouille::{Response, Request};

use crate::tmpl::TemplateContext;

/// Error that could occur when processing a request.
#[derive(Fail, Debug)]
pub enum WebError {
    /// The given entity was not found.
    #[fail(display = "not found")]
    NotFound,
    /// Request was missing a query parameter.
    #[fail(display = "query parameter missing")]
    QueryParameterMissing,
    /// RPC error.
    #[fail(display = "RPC: {}", _0)]
    Rpc(RpcError),
    /// SQL error from tspl-sqlite.
    #[fail(display = "tspl-sqlite: {}", _0)]
    Sql(SqlError),
    /// r2d2 database error.
    #[fail(display = "r2d2: {}", _0)]
    Pool(PoolError),
    /// Handlebars rendering error.
    #[fail(display = "handlebars: {}", _0)]
    Hbs(RenderError),
    /// reqwest error.
    #[fail(display = "reqwest: {}", _0)]
    Reqwest(ReqwestError)
}

impl WebError {
    pub fn as_rendered(&self, req: &Request, hbs: &Handlebars) -> Result<Response> {
        use self::WebError::*;
        use crate::templates::not_found::NotFoundView;
        use crate::templates::user_error::UserErrorView;

        let resp = match *self {
            NotFound => {
                TemplateContext {
                    template: "not_found",
                    title: "Not found".into(),
                    body: NotFoundView {
                        uri: req.url()
                    }
                }.render(hbs)?
            },
            QueryParameterMissing => {
                TemplateContext {
                    template: "user_error",
                    title: "Bad request (400)".into(),
                    body: UserErrorView {
                        error_summary: "Bad request (400)".into(),
                        reason: "Query parameter missing.".into()
                    }
                }.render(hbs)?

            },
            _ => {
                TemplateContext::title("ise", "").render(hbs)?
            }
        };
        Ok(resp.with_status_code(self.status_code()))
    }
}
impl StatusCode for WebError {
    fn status_code(&self) -> u16 {
        use self::WebError::*;

        match *self {
            NotFound => 404,
            QueryParameterMissing => 400,
            Rpc(ref r) => r.status_code(),
            Pool(_) => 503,
            _ => 500
        }
    }
}

impl_from_for_error!(WebError,
                     ReqwestError => Reqwest,
                     SqlError => Sql,
                     PoolError => Pool,
                     RenderError => Hbs,
                     RpcError => Rpc);

pub type WebResult<T> = ::std::result::Result<T, WebError>;
pub type Result<T, E = Error> = ::std::result::Result<T, E>;
