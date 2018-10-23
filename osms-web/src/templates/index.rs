use templates::movement_search::MovementSearchView;

#[derive(Serialize)]
pub struct IndexView {
    pub mvt_search: MovementSearchView
}
