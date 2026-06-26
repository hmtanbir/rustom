use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

/// Pagination parameters.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PaginationParams {
    pub fn get_page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn get_per_page(&self) -> u32 {
        self.per_page.unwrap_or(10).max(1)
    }

    pub fn offset(&self) -> u32 {
        (self.get_page() - 1) * self.get_per_page()
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(per_page) = self.per_page
            && per_page > 100 {
                let mut errors = HashMap::new();
                errors.insert(
                    "per_page".to_string(),
                    vec!["per_page must not exceed 100".to_string()],
                );
                return Err(AppError::Validation(errors));
            }
        Ok(())
    }
}

/// Paginated response metadata.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub status: u16,
    pub message: String,
    pub data: Vec<T>,
    pub current_page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub total_count: u32,
    pub next_page: Option<u32>,
    pub prev_page: Option<u32>,
}
