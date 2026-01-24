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

impl From<wreq::Error> for AppError {
    fn from(err: wreq::Error) -> Self {
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
        let user_friendly_error = error_to_user_message(&self.0);
        let body = crate::templates::error_page(user_friendly_error);
        Html(body).into_response()
    }
}

pub fn error_to_user_message(err: &anyhow::Error) -> String {
    let err_string = err.to_string();

    // Check for specific error patterns and convert to user-friendly messages
    if err_string.contains("username is required") {
        return "Please enter a Letterboxd username.".to_string();
    }

    if err_string.contains("country must be a 2-letter code") {
        return "Please select a valid country.".to_string();
    }

    if err_string.contains("404") || err_string.contains("Not Found") {
        // This could be a user not found or a film page not found
        if err_string.contains("letterboxd.com") {
            if err_string.contains("/watchlist/") {
                return "Letterboxd user not found. Please check the username and try again."
                    .to_string();
            } else if err_string.contains("/film/") {
                return "Unable to find film information. This film may no longer exist on \
                        Letterboxd."
                    .to_string();
            }
        }
    }

    if err_string.contains("TMDB API") || err_string.contains("themoviedb") {
        return "Unable to fetch movie data from TMDB. Please try again later.".to_string();
    }

    if err_string.contains("network") || err_string.contains("timeout") {
        return "Network error occurred. Please check your connection and try again.".to_string();
    }

    if err_string.contains("rate limit") {
        return "Too many requests. Please wait a moment and try again.".to_string();
    }

    // For any other errors, provide a generic message
    "An unexpected error occurred while processing your request. Please try again.".to_string()
}

pub type AppResult<T> = Result<T, AppError>;
