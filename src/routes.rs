use std::{convert::Infallible, sync::Arc};

use anyhow::Context;
use axum::{
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        Html, IntoResponse, Redirect, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use futures::{StreamExt, channel::mpsc};
use serde::Deserialize;
use time::Duration;
use tracing::{error, info};

use crate::{
    AppState, countries::COUNTRIES, error::AppResult, models::TrackRequest, processor::Progress,
    templates, watchlist,
};

const WATCHLIST_COOKIE: &str = "watchlist";
const COUNTRY_COOKIE: &str = "country";

pub async fn index(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> AppResult<Html<String>> {
    // Cloudflare sets CF-IPCountry from the client IP when the site is proxied through it; without
    // it the page falls back to the browser's timezone and language.
    let country = jar.get(COUNTRY_COOKIE).map(|c| c.value().to_string()).or_else(|| {
        headers
            .get("cf-ipcountry")
            .and_then(|v| v.to_str().ok())
            .filter(|code| COUNTRIES.iter().any(|c| c.code.eq_ignore_ascii_case(code)))
            .map(str::to_uppercase)
    });
    let saved = match jar.get(WATCHLIST_COOKIE) {
        Some(token) => watchlist::load(state.cache.db(), token.value()).await?,
        None => None,
    };

    Ok(Html(templates::index_page(saved.as_ref(), country.as_deref())))
}

pub async fn upload(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let mut country = String::new();
    let mut file = None;
    while let Some(field) = multipart.next_field().await.context("failed to read upload")? {
        match field.name() {
            Some("country") => country = field.text().await.context("failed to read upload")?,
            Some("file") => {
                let name = field.file_name().unwrap_or_default().to_string();
                let bytes = field.bytes().await.context("failed to read upload")?;
                file = Some((name, bytes));
            },
            _ => {},
        }
    }

    let country = country.trim().to_uppercase();
    if !is_country_code(&country) {
        return Ok(upload_error("Please select a valid country."));
    }
    let Some((file_name, bytes)) = file.filter(|(_, bytes)| !bytes.is_empty()) else {
        return Ok(upload_error("Choose your Letterboxd export ZIP or watchlist.csv to upload."));
    };

    let upload = match watchlist::parse_upload(&file_name, &bytes) {
        Ok(upload) => upload,
        Err(err) => return Ok(upload_error(&err.to_string())),
    };
    info!(film_count = upload.films.len(), "watchlist uploaded");

    let existing = jar.get(WATCHLIST_COOKIE).map(|c| c.value().to_string());
    let token = watchlist::save(state.cache.db(), existing.as_deref(), upload).await?;

    Ok((remember(jar, &token, &country), Redirect::to(&templates::results_url(&token, &country)))
        .into_response())
}

pub async fn track(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Query(req): Query<TrackRequest>,
) -> AppResult<(CookieJar, Html<String>)> {
    let token = req.watchlist.trim().to_string();
    let country = req.country.trim().to_uppercase();

    if !is_country_code(&country) {
        return Err(anyhow::anyhow!("country must be a 2-letter code").into());
    }
    if token.is_empty() {
        return Err(anyhow::anyhow!("saved watchlist not found").into());
    }

    let saved = watchlist::load(state.cache.db(), &token).await?;

    Ok((
        remember(jar, &token, &country),
        Html(templates::processing_page(&token, &country, saved.as_ref())),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ProcessQuery {
    watchlist: String,
    country: String,
}

/// Streams `progress` events while TMDB requests run, then one `done` event with the results HTML.
/// Processing continues if the browser disconnects, so the cache still fills.
pub async fn process(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ProcessQuery>,
) -> Response {
    let token = q.watchlist.trim().to_string();
    let country = q.country.trim().to_uppercase();

    info!(country = %country, "processing request");

    let (tx, rx) = mpsc::unbounded();
    tokio::spawn(async move {
        let progress_tx = tx.clone();
        let on_progress = move |progress: Progress| {
            if let Ok(event) = Event::default().event("progress").json_data(progress) {
                let _ = progress_tx.unbounded_send(event);
            }
        };

        let result = async {
            if !is_country_code(&country) {
                anyhow::bail!("country must be a 2-letter code");
            }

            let saved = watchlist::load(state.cache.db(), &token)
                .await?
                .ok_or_else(|| anyhow::anyhow!("saved watchlist not found"))?;

            let today: jiff::civil::Date = jiff::ZonedDateTime::now().into();
            let (films, unmatched) = crate::processor::process(
                &state.cache,
                &state.tmdb,
                saved.films.clone(),
                &country,
                state.config.max_concurrent,
                today.year(),
                &on_progress,
            )
            .await?;
            info!(
                result_count = films.len(),
                unmatched_count = unmatched.len(),
                "completed processing"
            );

            Ok::<_, anyhow::Error>(templates::results_fragment(
                &saved, &country, &films, &unmatched,
            ))
        }
        .await;

        let body = match result {
            Ok(html) => html,
            Err(err) => {
                error!(error = %err, "request failed");
                let user_friendly_error = crate::error::error_to_user_message(&err);
                templates::error_fragment(user_friendly_error)
            },
        };
        let _ = tx.unbounded_send(Event::default().event("done").data(body));
    });

    // Buffering or recompressing this stream delays progress until a large batch has accumulated.
    (
        [("x-accel-buffering", "no"), ("cache-control", "no-cache, no-transform")],
        Sse::new(rx.map(Ok::<_, Infallible>)).keep_alive(KeepAlive::default()),
    )
        .into_response()
}

fn is_country_code(country: &str) -> bool {
    country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic())
}

fn remember(jar: CookieJar, token: &str, country: &str) -> CookieJar {
    let cookie = |name, value: &str| {
        Cookie::build((name, value.to_string()))
            .path("/")
            .max_age(Duration::days(365))
            .same_site(cookie::SameSite::Lax)
            .http_only(true)
            .build()
    };
    jar.add(cookie(WATCHLIST_COOKIE, token)).add(cookie(COUNTRY_COOKIE, country))
}

fn upload_error(message: &str) -> Response {
    (StatusCode::BAD_REQUEST, Html(templates::error_page(message.to_string()))).into_response()
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    use super::*;
    use crate::{cache::CacheManager, config::Config, tmdb::TmdbClient};

    #[tokio::test]
    async fn progress_stream_disables_proxy_buffering_and_transformation() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let db = crate::db::connect_and_migrate("sqlite::memory:").await.unwrap();
        let config = Arc::new(Config {
            addr: "127.0.0.1:0".parse().unwrap(),
            tmdb_access_token: String::new(),
            tmdb_base_url: "https://api.themoviedb.org/3".to_string(),
            database_url: "sqlite::memory:".to_string(),
            cache_ttl_days: 7,
            release_cache_hours: 24,
            provider_cache_days: 7,
            tmdb_rps: 4,
            max_concurrent: 5,
        });
        let state = Arc::new(AppState {
            cache: CacheManager::new(db, 7, 24, 7),
            tmdb: Arc::new(TmdbClient::new(
                reqwest::Client::new(),
                String::new(),
                config.tmdb_base_url.clone(),
                4,
            )),
            config,
        });
        let response = Router::new()
            .route("/process", get(process))
            .with_state(state)
            .oneshot(
                Request::get("/process?watchlist=missing&country=AU").body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        assert_eq!(response.headers()["x-accel-buffering"], "no");
        assert_eq!(response.headers()["cache-control"], "no-cache, no-transform");
    }
}
