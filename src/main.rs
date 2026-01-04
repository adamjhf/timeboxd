mod cache;
mod config;
mod countries;
mod db;
mod entities;
mod error;
mod models;
mod processor;
mod routes;
mod scraper;
mod templates;
mod tmdb;

use std::{sync::Arc, time::Duration};

use axum::{Router, routing::get};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{cache::CacheManager, config::Config, tmdb::TmdbClient};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    pub cache: CacheManager,
    pub tmdb: Arc<TmdbClient>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,timeboxd=debug,sqlx=warn".to_string()),
        )
        .init();

    let config = Arc::new(Config::from_env()?);

    let http = reqwest::Client::builder()
        .user_agent("timeboxd/0.1")
        .timeout(Duration::from_secs(30))
        .build()?;

    let db = db::connect_and_migrate(&config.database_url).await?;
    let cache = CacheManager::new(db, config.cache_ttl_days);

    let tmdb = TmdbClient::new(
        http.clone(),
        config.tmdb_access_token.clone(),
        config.tmdb_base_url.clone(),
        config.tmdb_rps,
    );

    let state = Arc::new(AppState { config: config.clone(), http, cache, tmdb: Arc::new(tmdb) });

    let app = Router::new()
        .route("/", get(routes::index))
        .route("/track", axum::routing::post(routes::track))
        .route("/process", get(routes::process))
        .with_state(state)
        .layer(CorsLayer::new().allow_origin(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    tracing::info!(addr = %config.addr, "listening");
    axum::serve(listener, app).await?;

    Ok(())
}
