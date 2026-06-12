pub mod v1;

use axum::Router;
use crate::app_state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .nest("/v1", v1::routes())
}
