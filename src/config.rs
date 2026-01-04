use std::net::SocketAddr;

use anyhow::Context;

#[derive(Clone, Debug)]
pub struct Config {
    pub addr: SocketAddr,
    pub tmdb_api_key: String,
    pub tmdb_base_url: String,
    pub database_url: String,
    pub cache_ttl_days: i64,
    pub tmdb_rps: u32,
    pub max_concurrent: usize,
    pub letterboxd_delay_ms: u64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port: u16 =
            std::env::var("PORT").unwrap_or_else(|_| "3000".to_string()).parse().context("PORT")?;

        let tmdb_api_key = std::env::var("TMDB_API_KEY").unwrap_or_else(|_| "".to_string());
        let tmdb_base_url = std::env::var("TMDB_BASE_URL")
            .unwrap_or_else(|_| "https://api.themoviedb.org/3".to_string());

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://timeboxd.db?mode=rwc".to_string());

        let cache_ttl_days: i64 =
            std::env::var("CACHE_TTL_DAYS").ok().and_then(|s| s.parse().ok()).unwrap_or(7);

        let tmdb_rps: u32 =
            std::env::var("TMDB_RPS").ok().and_then(|s| s.parse().ok()).unwrap_or(4);

        let max_concurrent: usize =
            std::env::var("MAX_CONCURRENT_REQUESTS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);

        let letterboxd_delay_ms: u64 =
            std::env::var("LETTERBOXD_DELAY_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(250);

        Ok(Self {
            addr: format!("{host}:{port}").parse().context("HOST/PORT")?,
            tmdb_api_key,
            tmdb_base_url,
            database_url,
            cache_ttl_days,
            tmdb_rps,
            max_concurrent,
            letterboxd_delay_ms,
        })
    }
}
