use std::sync::Arc;

use axum::{
    extract::{Form, Query, State},
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;

use crate::{AppState, error::AppResult, models::TrackRequest, templates};

pub async fn index() -> Html<String> {
    Html(templates::index_page())
}

pub async fn track(Form(req): Form<TrackRequest>) -> AppResult<Html<String>> {
    let username = req.username.trim().to_string();
    let country = req.country.trim().to_uppercase();

    if username.is_empty() {
        return Err(anyhow::anyhow!("username is required").into());
    }

    if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(anyhow::anyhow!("country must be a 2-letter code").into());
    }

    Ok(Html(templates::processing_page(&username, &country)))
}

#[derive(Debug, Deserialize)]
pub struct ProcessQuery {
    username: String,
    country: String,
}

pub async fn process(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ProcessQuery>,
) -> Response {
    let username = q.username.trim().to_string();
    let country = q.country.trim().to_uppercase();

    let result = async {
        if username.is_empty() {
            anyhow::bail!("username is required");
        }
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
            anyhow::bail!("country must be a 2-letter code");
        }

        let today: jiff::civil::Date = jiff::Zoned::now().into();
        let current_year = today.year();
        let cutoff_year = current_year.saturating_sub(3);

        let watchlist = crate::scraper::fetch_watchlist(
            &state.http,
            &username,
            state.config.letterboxd_delay_ms,
            cutoff_year,
        )
        .await?;

        let films = crate::processor::process(
            &state.http,
            &state.cache,
            &*state.tmdb,
            watchlist,
            &country,
            state.config.max_concurrent,
            current_year,
        )
        .await?;

        Ok::<_, anyhow::Error>(templates::results_fragment(&username, &country, &films))
    }
    .await;

    let body = match result {
        Ok(html) => html,
        Err(err) => templates::error_fragment(err.to_string()),
    };

    let mut resp = Html(body).into_response();
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut()
        .insert("datastar-selector", HeaderValue::from_static("#content"));
    resp.headers_mut()
        .insert("datastar-mode", HeaderValue::from_static("outer"));
    resp
}
