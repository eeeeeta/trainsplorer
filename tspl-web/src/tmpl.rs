//! Templating engine stuff.
//!
//! Mostly copied from the old osms-web.

use serde::Serialize;
use std::borrow::Cow;
use handlebars::Handlebars;
use rouille::Response;
use serde_derive::Serialize;
use log::*;

use crate::errors::*;

#[derive(Serialize)]
pub struct TemplateContext<'a, T> where T: Serialize {
    pub template: &'static str,
    pub title: Cow<'a, str>,
    pub body: T
}
impl<'a, T> TemplateContext<'a, T> where T: Serialize {
    pub fn render(self, hbs: &Handlebars) -> WebResult<Response> {
        match hbs.render(self.template, &self) {
            Ok(d) => Ok(Response::html(d)),
            Err(e) => {
                warn!("Failed to render template: {}", e);
                Err(e)?
            }
        }
    }
}
impl<'a> TemplateContext<'a, ()> {
    pub fn title<U: Into<Cow<'a, str>>>(template: &'static str, title: U) -> Self {
        TemplateContext {
            template,
            title: title.into(),
            body: ()
        }
    }
}
struct Partial {
    name: &'static str,
    content: &'static str
}
macro_rules! partial {
    ($name:expr) => {
        Partial { 
            name: $name,
            content: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/", $name, ".html.hbs"))
        }
    }
}

// *** Instructions for adding a new partial ***
// - 1. Create templates/NAME.html.hbs and src/templates/NAME.rs.
// - 2. Add partial!("NAME") to the below array.
// - 3. Increment the array length.
// - 4. Put your HTML in the HBS file and all your views in the Rust one.
// - 5. ???
// - 6. Profit!
static PARTIALS: [Partial; 16] = [
    partial!("connections"),
    partial!("footer"),
    partial!("header"),
    partial!("index"),
    partial!("ise"),
    partial!("map"),
    partial!("movement_search"),
    partial!("movements"),
    partial!("nav"),
    partial!("not_found"),
    partial!("orig_dest"),
    partial!("schedule"),
    partial!("schedules"),
    partial!("user_error"),
    partial!("train"),
    partial!("symbols_guide")
];
pub fn handlebars_init() -> Result<Handlebars> {
    let mut hbs = Handlebars::new();
    hbs.set_strict_mode(true);
    for partial in PARTIALS.iter() {
        hbs.register_partial(partial.name, partial.content)?;
    }
    Ok(hbs)
}
