//! Main server context.

use tspl_sqlite::TsplPool;
use handlebars::Handlebars;
use rouille::{Request, Response, router};
use std::time::Instant;
use chrono::prelude::*;
use chrono::Duration;
use log::*;

use crate::config::Config;
use crate::tmpl::TemplateContext;
use crate::errors::*;
use crate::suggestions::*;
use crate::movements::*;

pub struct App {
    hbs: Handlebars,
    ss: StationSuggester,
    ms: MovementSearcher
}
impl App {
    pub fn new(cfg: &Config, pool: TsplPool, hbs: Handlebars) -> Self {
        let ss = StationSuggester::new(pool.clone());
        let ms = MovementSearcher::new(pool, cfg);
        App { hbs, ss, ms }
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
            (POST) (/movements) => {
                let input = rouille::input::post::raw_urlencoded_post_input(req)
                    .map_err(|e| e.to_string())?;
                let (mut tiploc, mut date, mut time) = (None, None, None);
                for (k, v) in input {
                    match &k as &str {
                        "ts-tiploc" => {
                            if v.trim().len() == 0 {
                                Err("Location cannot be blank.".to_string())?;
                            }
                            else {
                                tiploc = Some(v);
                            }
                        },
                        "ts-date" => {
                            let d = NaiveDate::parse_from_str(&v, "%Y-%m-%d")
                                .map_err(|e| format!("Error parsing date: {}", e))?;
                            date = Some(d);
                        },
                        "ts-time" => {
                            let t = NaiveTime::parse_from_str(&v, "%H:%M")
                                .map_err(|e| format!("Error parsing time: {}", e))?;
                            time = Some(t);
                        },
                        _ => {}
                    }
                }
                let tiploc = tiploc.ok_or("A location is required.".to_string())?;
                let date = date.ok_or("A date is required.".to_string())?;
                let time = time.ok_or("A time is required.".to_string())?;
                Ok(Response::redirect_303(format!("/movements/{}/{}/{}", tiploc, date, time)))
            },
            (GET) (/movements/{tpl: String}/{date: NaiveDate}/{time: NaiveTime}) => {
                use crate::templates::movements::MovementsView;
                use crate::templates::movement_search::MovementSearchView;

                let dur = Duration::hours(1);
                let ts = date.and_time(time);
                let descs = self.ms.get_movements_through_tiploc(&tpl, ts, dur)?;
                Ok(TemplateContext {
                    template: "movements",
                    title: "Movement search".into(),
                    body: MovementsView {
                        mvts: descs,
                        mvt_search: MovementSearchView {
                            error: None,
                            station: Some(tpl),
                            date: ts.date().format("%Y-%m-%d").to_string(),
                            time: ts.time().format("%H:%M").to_string()
                        }
                    }
                }.render(&self.hbs)?)
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
