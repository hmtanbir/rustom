use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct UserQueryParams {
    pub email: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub uuid: Option<String>,
    pub deleted: Option<bool>,
    pub order: Option<String>,
}
