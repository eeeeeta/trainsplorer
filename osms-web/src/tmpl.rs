use serde::Serialize;
use std::borrow::Cow;

#[derive(Serialize)]
pub struct TemplateContext<'a, T> where T: Serialize {
    pub title: Cow<'a, str>,
    pub body: T
}
impl<'a> TemplateContext<'a, ()> {
    pub fn title<U: Into<Cow<'a, str>>>(title: U) -> Self {
        TemplateContext {
            title: title.into(),
            body: ()
        }
    }
}
