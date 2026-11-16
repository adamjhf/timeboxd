mod cache;
mod config;
mod countries;
mod db;
mod entities;
mod error;
mod models;
mod processor;
mod routes;
mod templates;
mod tmdb;
mod watchlist;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::{cache::CacheManager, config::Config, tmdb::TmdbClient};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache: CacheManager,
    pub tmdb: Arc<TmdbClient>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,timeboxd=debug,tower_http=debug,sqlx=warn".to_string()),
        )
        .init();

    let config = Arc::new(Config::from_env()?);

    // ring is already used by sqlx; reqwest's default provider would add aws-lc, a large C build.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install rustls crypto provider"))?;

    let http = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;

    let db = db::connect_and_migrate(&config.database_url).await?;
    let cache = CacheManager::new(
        db,
        config.cache_ttl_days,
        config.release_cache_hours,
        config.provider_cache_days,
    );

    let tmdb = TmdbClient::new(
        http,
        config.tmdb_access_token.clone(),
        config.tmdb_base_url.clone(),
        config.tmdb_rps,
    );

    let state = Arc::new(AppState { config: config.clone(), cache, tmdb: Arc::new(tmdb) });

    let app = Router::new()
        .route("/", get(routes::index))
        .route("/upload", post(routes::upload).layer(DefaultBodyLimit::max(25 * 1024 * 1024)))
        .route("/release-dates", get(routes::track))
        .route("/process", get(routes::process))
        .with_state(state)
        .layer(CorsLayer::new().allow_origin(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    info!(addr = %config.addr, "server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
