use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use serde::Deserialize;
use time::Duration;
use tracing::{error, info};

use crate::{AppState, error::AppResult, models::TrackRequest, templates};

pub async fn index(jar: CookieJar) -> Html<String> {
    let username = jar.get("username").map(|c| c.value().to_string());
    let country = jar.get("country").map(|c| c.value().to_string());

    Html(templates::index_page(username.as_deref(), country.as_deref()))
}

pub async fn track(
    jar: CookieJar,
    Query(req): Query<TrackRequest>,
) -> AppResult<(CookieJar, Html<String>)> {
    let username = req.username.trim().to_string();
    let country = req.country.trim().to_uppercase();

    if username.is_empty() {
        return Err(anyhow::anyhow!("username is required").into());
    }

    if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(anyhow::anyhow!("country must be a 2-letter code").into());
    }

    let max_age = Duration::days(365);

    let username_cookie = Cookie::build(("username", username.clone()))
        .path("/")
        .max_age(max_age)
        .same_site(cookie::SameSite::Lax)
        .build();

    let country_cookie = Cookie::build(("country", country.clone()))
        .path("/")
        .max_age(max_age)
        .same_site(cookie::SameSite::Lax)
        .build();

    let jar = jar.add(username_cookie).add(country_cookie);

    Ok((jar, Html(templates::processing_page(&username, &country))))
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

    info!(username = %username, country = %country, "processing request");

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
        info!(username = %username, film_count = watchlist.len(), "fetched watchlist");

        if watchlist.is_empty() {
            info!(username = %username, "empty watchlist");
            return Ok(templates::results_fragment(&username, &country, &[]));
        }

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
        info!(username = %username, result_count = films.len(), "completed processing");

        Ok::<_, anyhow::Error>(templates::results_fragment(&username, &country, &films))
    }
    .await;

    let body = match result {
        Ok(html) => html,
        Err(err) => {
            error!(username = %username, error = %err, "request failed");
            let user_friendly_error = crate::error::error_to_user_message(&err);
            templates::error_fragment(user_friendly_error)
        },
    };

    let mut resp = Html(body).into_response();
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert("datastar-selector", HeaderValue::from_static("#content"));
    resp.headers_mut().insert("datastar-mode", HeaderValue::from_static("outer"));
    resp
}
