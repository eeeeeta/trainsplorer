use crate::templates::movement_search::MovementSearchView;
use serde_derive::Serialize;

#[derive(Serialize)]
pub struct IndexView {
    pub mvt_search: MovementSearchView
}
