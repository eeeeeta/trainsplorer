use handlebars::Handlebars;
use pool::Pool;
use std::sync::Arc;
use rouille::{Request, Response};
use tmpl::TemplateContext;
use chrono::Local;
use rouille;
use schedules;
use schedule;
use movements;
use mapping;

pub type Sctx = Arc<ServerContext>;

#[derive(Serialize)]
pub struct NotFoundView {
    uri: String
}
#[derive(Serialize)]
pub struct UserErrorView {
    error_summary: String,
    reason: String
}
#[derive(Serialize)]
pub struct MovementSearchView {
    pub error: Option<String>,
    pub station: Option<String>,
    pub date: String,
    pub time: String
}
#[derive(Serialize)]
pub struct IndexView {
    mvt_search: MovementSearchView
}

pub struct ServerContext {
    pub hbs: Handlebars,
    pub db: Pool
}
macro_rules! try_or_ise {
    ($sctx:expr, $res:expr) => {
        match $res {
            Ok(r) => r,
            Err(e) => {
                return ::sctx::ServerContext::handle_ise($sctx.clone(), e.into());
            }
        }
    };
}
macro_rules! try_or_badreq {
    ($sctx:expr, $res:expr) => {
        match $res {
            Ok(r) => r,
            Err(e) => {
                return ::sctx::ServerContext::user_error($sctx.clone(), "Bad request (400)".into(), e.to_string(), 400);
            }
        }
    };
}
macro_rules! try_or_nonexistent {
    ($sctx:expr, $res:expr) => {
        match $res {
            Ok(r) => r,
            Err(e) => {
                return ::sctx::ServerContext::user_error($sctx.clone(), "Item not found (404)".into(), e.to_string(), 404);
            }
        }
    };
}
macro_rules! get_db {
    ($sctx:expr) => {
        match $sctx.db.get() {
            Ok(r) => r,
            Err(_) => {
                return ::sctx::ServerContext::overloaded($sctx.clone());
            }
        }
    }
}
macro_rules! render {
    ($sctx:expr, $tctx:expr) => {{
        let ret = try_or_ise!($sctx, $tctx.render(&$sctx.hbs));
        ret
    }};
}
impl ServerContext {
    pub fn new(hbs: Handlebars, db: Pool) -> Arc<Self> {
        Arc::new(Self { hbs, db })
    }
    pub fn handle_ise(selfish: Arc<Self>, e: ::failure::Error) -> Response {
        error!("500 ISE: {}", e);
        for cause in e.iter_chain() {
            error!("Caused by: {}", cause);
        }
        error!("Backtrace: {}", e.backtrace());
        let tctx = TemplateContext::title("ise", "");
        match tctx.render(&selfish.hbs) {
            Ok(h) => h.with_status_code(500),
            Err(e) => {
                error!("Failed to render ISE template (!!): {}", e);
                Response::text("everything is broken :(")
                    .with_status_code(500)
            }
        }
    }
    pub fn index(selfish: Arc<Self>) -> Response {
        let now = Local::now();
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M").to_string();
        let tctx = TemplateContext {
            template: "index",
            title: "Home".into(),
            body: IndexView {
                mvt_search: MovementSearchView {
                    error: None,
                    station: None,
                    date,
                    time
                }
            }
        };
        render!(selfish, tctx)
    }
    pub fn overloaded(selfish: Arc<Self>) -> Response {
        error!("503 - unable to get DB connection!");
        let tctx = TemplateContext::title("overloaded", "");
        render!(selfish, tctx).with_status_code(503)
    }
    pub fn user_error(selfish: Arc<Self>, error_summary: String, reason: String, code: u16) -> Response {
        let tctx = TemplateContext {
            template: "user_error",
            title: "".into(),
            body: UserErrorView {
                error_summary,
                reason
            }
        };
        render!(selfish, tctx).with_status_code(code)
    }
    pub fn not_found(selfish: Arc<Self>, req: &Request) -> Response {
        let tctx = TemplateContext {
            template: "not_found",
            title: "".into(),
            body: NotFoundView {
                uri: req.url()
            }
        };
        render!(selfish, tctx).with_status_code(404)
    }
    pub fn handle(selfish: Arc<Self>, req: &Request) -> Response {
        let ret = router!(req,
            (GET) (/) => {
                ServerContext::index(selfish.clone())
            },
            (GET) (/schedules) => {
                schedules::schedules(selfish.clone(), req)
            },
            (GET) (/train/{id: i32}) => {
                schedule::train(selfish.clone(), id)
            },
            (GET) (/schedule/{id: i32}) => {
                schedule::schedule(selfish.clone(), id)
            },
            (POST) (/movements) => {
                movements::post_index_movements(selfish.clone(), req)
            },
            (GET) (/station_suggestions) => {
                movements::station_suggestions(selfish.clone(), req)
            },
            (GET) (/movements/{station}/{date}/{time}) => {
                movements::movements(selfish.clone(), station, date, time)
            },
            (GET) (/map) => {
                mapping::map(selfish.clone())
            },
            (POST) (/geo/correct_station) => {
                mapping::geo_correct_station(selfish.clone(), req)
            },
            (GET) (/geo/stations) => {
                mapping::geo_stations(selfish.clone(), req)
            },
            (GET) (/geo/ways) => {
                mapping::geo_ways(selfish.clone(), req)
            },
            _ => {
                let asset_resp = rouille::match_assets(req, "static");
                if asset_resp.is_success() {
                    asset_resp
                }
                else {
                    ServerContext::not_found(selfish.clone(), req)
                }
            }
        );
        info!("{} {} \"{}\" - {}", req.remote_addr(), req.method(), req.raw_url(), ret.status_code);
        ret
    }
}
