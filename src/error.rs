use axum::response::{Html, IntoResponse, Response};

#[derive(Debug)]
pub struct AppError(anyhow::Error);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for AppError {}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self(err)
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self(anyhow::Error::new(err))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        Self(anyhow::Error::new(err))
    }
}

impl From<jiff::Error> for AppError {
    fn from(err: jiff::Error) -> Self {
        Self(anyhow::Error::new(err))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = crate::templates::error_page(self.to_string());
        Html(body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
