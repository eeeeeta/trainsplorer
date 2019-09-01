//! Main server context.

use tspl_sqlite::TsplPool;
use tspl_util::rpc::MicroserviceRpc;
use tspl_util::user_agent;
use handlebars::Handlebars;
use rouille::{Request, Response, router};
use std::time::Instant;
use chrono::prelude::*;
use log::*;

use crate::config::Config;
use crate::tmpl::TemplateContext;
use crate::errors::*;
use crate::suggestions::*;

pub struct App {
    hbs: Handlebars,
    ss: StationSuggester,
    frpc: MicroserviceRpc,
    zrpc: MicroserviceRpc,
    vrpc: MicroserviceRpc
}
impl App {
    pub fn new(cfg: &Config, pool: TsplPool, hbs: Handlebars) -> Self {
        let frpc = MicroserviceRpc::new(user_agent!(), "fahrplan", cfg.service_fahrplan.clone());
        let zrpc = MicroserviceRpc::new(user_agent!(), "zugfuhrer", cfg.service_zugfuhrer.clone());
        let vrpc = MicroserviceRpc::new(user_agent!(), "verknupfen", cfg.service_verknupfen.clone());
        let ss = StationSuggester::new(pool);
        App { hbs, ss, frpc, zrpc, vrpc }
    }
    fn on_request(&self, req: &Request) -> WebResult<Response> {
        router!(req,
            (GET) (/) => {
                use crate::templates::index::IndexView;
                use crate::templates::movement_search::MovementSearchView;

                let now = Local::now();
                let tctx = TemplateContext {
                    template: "index",
                    title: "Welcome to trainsplorer".into(),
                    body: IndexView {
                        mvt_search: MovementSearchView {
                            error: None,
                            station: None,
                            date: now.format("%Y-%m-%d").to_string(),
                            time: now.format("%H:%M").to_string()
                        }
                    }
                };
                Ok(tctx.render(&self.hbs)?)
            },
            (GET) (/station_suggestions) => {
                let query = req.get_param("query")
                    .ok_or(WebError::QueryParameterMissing)?;
                let ret = self.ss.suggestions_for(&query)?;
                Ok(Response::json(&ret))
            },
            _ => {
                let asset_resp = rouille::match_assets(req, "static");
                if asset_resp.is_success() {
                    Ok(asset_resp)
                }
                else {
                    Err(WebError::NotFound)
                }
            }
        )
    }
    pub fn handle_request(&self, req: &Request) -> Response {
        let start = Instant::now();
        let ret = self.on_request(req);
        let ret = match ret {
            Ok(r) => r,
            Err(e) => {
                warn!("Processing request failed: {}", e);
                let resp = e.as_rendered(req, &self.hbs);
                match resp {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Rendering error response failed: {}", e);
                        Response::text("Something's catastrophically broken!")
                            .with_status_code(500)
                    }
                }
            }
        };
        let dur = start.elapsed();
        info!("{} {} \"{}\" - {} [{}.{:03}s]", req.remote_addr(), req.method(), req.raw_url(), ret.status_code, dur.as_secs(), dur.subsec_millis());
        ret
    }
}
