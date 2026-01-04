use std::{num::NonZeroU32, sync::Arc};

use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use jiff::{civil::Date, fmt::temporal::DateTimeParser};
use serde::Deserialize;

use crate::{
    error::AppResult,
    models::{ReleaseDate, ReleaseType},
};

pub struct TmdbClient {
    client: reqwest::Client,
    access_token: String,
    base_url: String,
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl TmdbClient {
    pub fn new(client: reqwest::Client, access_token: String, base_url: String, rps: u32) -> Self {
        // Warn once on app load if using mock data
        if access_token.trim().is_empty() {
            tracing::warn!("Using mock TMDB data - no TMDB_ACCESS_TOKEN provided");
        }

        let limiter =
            Arc::new(RateLimiter::direct(Quota::per_second(NonZeroU32::new(rps.max(1)).unwrap())));
        Self { client, access_token, base_url, limiter }
    }

    pub async fn search_movie(&self, title: &str, year: Option<i16>) -> AppResult<Option<i32>> {
        // Use mock data if access token is not provided
        if self.access_token.trim().is_empty() {
            return Ok(Some(550)); // Mock TMDB ID for Fight Club
        }

        self.limiter.until_ready().await;

        let url = format!("{}/search/movie", self.base_url.trim_end_matches('/'));
        let mut req = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .query(&[("query", &title.to_string())]);
        if let Some(year) = year {
            req = req.query(&[("year", year)]);
        }

        let resp: SearchResponse = req.send().await?.error_for_status()?.json().await?;
        Ok(resp.results.into_iter().next().map(|m| m.id))
    }

    pub async fn get_release_dates(
        &self,
        tmdb_id: i32,
        country: &str,
    ) -> AppResult<(Vec<ReleaseDate>, Vec<ReleaseDate>)> {
        // Use mock data if access token is not provided
        if self.access_token.trim().is_empty() {
            let today: Date = jiff::Zoned::now().into();
            let future_date = today + jiff::Span::new().years(1); // 1 year from now

            let theatrical = vec![ReleaseDate {
                date: future_date,
                release_type: ReleaseType::Theatrical,
                note: Some("Mock theatrical release".to_string()),
            }];

            let streaming = vec![ReleaseDate {
                date: future_date + jiff::Span::new().months(3), // 3 months later
                release_type: ReleaseType::Digital,
                note: Some("Mock streaming release".to_string()),
            }];

            return Ok((theatrical, streaming));
        }

        self.limiter.until_ready().await;

        let url =
            format!("{}/movie/{}/release_dates", self.base_url.trim_end_matches('/'), tmdb_id);

        let resp: ReleaseDatesResponse = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let today: Date = jiff::Zoned::now().into();

        let mut theatrical = Vec::new();
        let mut streaming = Vec::new();

        for res in resp.results {
            if res.iso_3166_1 != country {
                continue;
            }
            for rd in res.release_dates {
                let Some(kind) = ReleaseType::from_tmdb_code(rd.type_) else {
                    continue;
                };
                let timestamp =
                    DateTimeParser::new().parse_timestamp(rd.release_date.as_bytes())?;
                let date: Date = timestamp.to_zoned(jiff::tz::TimeZone::UTC).date();
                if date < today {
                    continue;
                }
                let note = rd.note.and_then(|s| {
                    let s = s.trim();
                    (!s.is_empty()).then(|| s.to_string())
                });
                let out = ReleaseDate { date, release_type: kind, note };
                match kind {
                    ReleaseType::Theatrical => theatrical.push(out),
                    ReleaseType::Digital => streaming.push(out),
                }
            }
        }

        theatrical.sort_by_key(|r| r.date);
        streaming.sort_by_key(|r| r.date);

        theatrical.dedup_by_key(|r| (r.date, r.release_type.as_tmdb_code(), r.note.clone()));
        streaming.dedup_by_key(|r| (r.date, r.release_type.as_tmdb_code(), r.note.clone()));

        Ok((theatrical, streaming))
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<SearchMovie>,
}

#[derive(Debug, Deserialize)]
struct SearchMovie {
    id: i32,
}

#[derive(Debug, Deserialize)]
struct ReleaseDatesResponse {
    results: Vec<ReleaseDatesCountry>,
}

#[derive(Debug, Deserialize)]
struct ReleaseDatesCountry {
    iso_3166_1: String,
    release_dates: Vec<ReleaseDateEntry>,
}

#[derive(Debug, Deserialize)]
struct ReleaseDateEntry {
    #[serde(rename = "release_date")]
    release_date: String,
    #[serde(rename = "type")]
    type_: i32,
    note: Option<String>,
}
