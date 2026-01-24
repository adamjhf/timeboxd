use std::{num::NonZeroU32, sync::Arc};

use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use jiff::{civil::Date, fmt::temporal::DateTimeParser};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::{
    error::AppResult,
    models::{
        CountryReleases, ProviderType, ReleaseDate, ReleaseDatesResult, ReleaseType, WatchProvider,
    },
};

pub struct TmdbClient {
    client: wreq::Client,
    access_token: String,
    base_url: String,
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl TmdbClient {
    pub fn new(client: wreq::Client, access_token: String, base_url: String, rps: u32) -> Self {
        if access_token.trim().is_empty() {
            warn!("TMDB_ACCESS_TOKEN not provided, using mock data");
        }

        let limiter =
            Arc::new(RateLimiter::direct(Quota::per_second(NonZeroU32::new(rps.max(1)).unwrap())));
        Self { client, access_token, base_url, limiter }
    }

    pub async fn search_movie(
        &self,
        title: &str,
        year: Option<i16>,
    ) -> AppResult<Option<(i32, Option<String>)>> {
        if self.access_token.trim().is_empty() {
            return Ok(Some((550, None)));
        }

        self.limiter.until_ready().await;

        debug!(title = %title, year = ?year, "TMDB API: searching movie");

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
        let result = resp.results.into_iter().next().map(|m| (m.id, m.poster_path));
        debug!(title = %title, result = ?result, "TMDB API: search result");
        Ok(result)
    }

    pub async fn get_movie_details(&self, tmdb_id: i32) -> AppResult<Option<String>> {
        if self.access_token.trim().is_empty() {
            return Ok(None);
        }

        self.limiter.until_ready().await;

        debug!(tmdb_id = tmdb_id, "TMDB API: fetching movie details");

        let url = format!("{}/movie/{}", self.base_url.trim_end_matches('/'), tmdb_id);

        let resp: MovieDetails = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        debug!(tmdb_id = tmdb_id, poster_path = ?resp.poster_path, "TMDB API: movie details result");
        Ok(resp.poster_path)
    }

    pub async fn get_release_dates(
        &self,
        tmdb_id: i32,
        country: &str,
    ) -> AppResult<ReleaseDatesResult> {
        // Use mock data if access token is not provided
        if self.access_token.trim().is_empty() {
            let today: Date = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC).date();
            let future_date = today + jiff::Span::new().years(1);

            let theatrical = vec![ReleaseDate {
                date: future_date,
                release_type: ReleaseType::Theatrical,
                note: Some("Mock theatrical release".to_string()),
            }];

            let streaming = vec![ReleaseDate {
                date: future_date + jiff::Span::new().months(3),
                release_type: ReleaseType::Digital,
                note: Some("Mock streaming release".to_string()),
            }];

            return Ok(ReleaseDatesResult {
                requested_country: CountryReleases {
                    country: country.to_string(),
                    theatrical,
                    streaming,
                },
                all_countries: vec![],
            });
        }

        self.limiter.until_ready().await;

        debug!(tmdb_id = tmdb_id, country = %country, "TMDB API: fetching release dates");

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

        let mut all_countries = Vec::new();

        for res in resp.results {
            let country_code = res.iso_3166_1.clone();
            let mut theatrical_future = Vec::new();
            let mut streaming_future = Vec::new();
            let mut theatrical_past = Vec::new();
            let mut streaming_past = Vec::new();

            for rd in res.release_dates {
                let Some(kind) = ReleaseType::from_tmdb_code(rd.type_) else {
                    continue;
                };
                let timestamp =
                    DateTimeParser::new().parse_timestamp(rd.release_date.as_bytes())?;
                let date: Date = timestamp.to_zoned(jiff::tz::TimeZone::UTC).date();
                let note = rd.note.and_then(|s| {
                    let s = s.trim();
                    (!s.is_empty()).then(|| s.to_string())
                });
                let out = ReleaseDate { date, release_type: kind, note };

                if date >= today {
                    match kind {
                        ReleaseType::Theatrical => theatrical_future.push(out),
                        ReleaseType::Digital => streaming_future.push(out),
                    }
                } else {
                    match kind {
                        ReleaseType::Theatrical => theatrical_past.push(out),
                        ReleaseType::Digital => streaming_past.push(out),
                    }
                }
            }

            theatrical_future.sort_by_key(|r| r.date);
            streaming_future.sort_by_key(|r| r.date);
            theatrical_past.sort_by_key(|r| r.date);
            streaming_past.sort_by_key(|r| r.date);

            theatrical_future
                .dedup_by_key(|r| (r.date, r.release_type.as_tmdb_code(), r.note.clone()));
            streaming_future
                .dedup_by_key(|r| (r.date, r.release_type.as_tmdb_code(), r.note.clone()));

            let _has_future_theatrical = !theatrical_future.is_empty();
            let _has_future_streaming = !streaming_future.is_empty();
            let has_past_theatrical = !theatrical_past.is_empty();
            let has_past_streaming = !streaming_past.is_empty();

            let mut theatrical = theatrical_future;
            let mut streaming = streaming_future;

            // Only include "Already available" if the latest release is within the last 2 years
            let two_years_ago = today - jiff::Span::new().years(2);

            if has_past_theatrical && theatrical.is_empty() {
                if let Some(latest) = theatrical_past.into_iter().max_by_key(|r| r.date) {
                    if latest.date >= two_years_ago {
                        theatrical.push(ReleaseDate {
                            date: latest.date,
                            release_type: ReleaseType::Theatrical,
                            note: Some("Already available".to_string()),
                        });
                    }
                }
            }

            if has_past_streaming && streaming.is_empty() {
                if let Some(latest) = streaming_past.into_iter().max_by_key(|r| r.date) {
                    if latest.date >= two_years_ago {
                        streaming.push(ReleaseDate {
                            date: latest.date,
                            release_type: ReleaseType::Digital,
                            note: Some("Already available".to_string()),
                        });
                    }
                }
            }

            all_countries.push(CountryReleases { country: country_code, theatrical, streaming });
        }

        let requested_country =
            all_countries.iter().find(|c| c.country == country).cloned().unwrap_or_else(|| {
                CountryReleases {
                    country: country.to_string(),
                    theatrical: vec![],
                    streaming: vec![],
                }
            });

        debug!(
            tmdb_id = tmdb_id,
            country = %country,
            all_countries_count = all_countries.len(),
            requested_theatrical = requested_country.theatrical.len(),
            requested_streaming = requested_country.streaming.len(),
            "TMDB API: release dates result"
        );

        Ok(ReleaseDatesResult { requested_country, all_countries })
    }

    pub async fn get_watch_providers(
        &self,
        tmdb_id: i32,
        country: &str,
    ) -> AppResult<(Vec<WatchProvider>, Option<String>)> {
        if self.access_token.trim().is_empty() {
            return Ok((
                vec![WatchProvider {
                    provider_id: 8,
                    provider_name: "Netflix".to_string(),
                    logo_path: "/pbpMk2JmcoNnQwx5JGpXngfoWtp.jpg".to_string(),
                    link: None,
                    provider_type: ProviderType::Stream,
                }],
                Some("https://www.themoviedb.org/movie/550/watch".to_string()),
            ));
        }

        self.limiter.until_ready().await;

        debug!(tmdb_id = tmdb_id, country = %country, "TMDB API: fetching watch providers");

        let url =
            format!("{}/movie/{}/watch/providers", self.base_url.trim_end_matches('/'), tmdb_id);

        let resp: WatchProvidersResponse = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let country_data = resp.results.get(country);

        let (providers, link) = match country_data {
            Some(data) => {
                let mut providers = Vec::new();

                if let Some(flatrate) = &data.flatrate {
                    for p in flatrate {
                        providers.push(WatchProvider {
                            provider_id: p.provider_id,
                            provider_name: p.provider_name.clone(),
                            logo_path: p.logo_path.clone(),
                            link: data.link.clone(),
                            provider_type: ProviderType::Stream,
                        });
                    }
                }

                if let Some(rent) = &data.rent {
                    for p in rent {
                        if !providers.iter().any(|existing| existing.provider_id == p.provider_id) {
                            providers.push(WatchProvider {
                                provider_id: p.provider_id,
                                provider_name: p.provider_name.clone(),
                                logo_path: p.logo_path.clone(),
                                link: data.link.clone(),
                                provider_type: ProviderType::Rent,
                            });
                        }
                    }
                }

                if let Some(buy) = &data.buy {
                    for p in buy {
                        if !providers.iter().any(|existing| existing.provider_id == p.provider_id) {
                            providers.push(WatchProvider {
                                provider_id: p.provider_id,
                                provider_name: p.provider_name.clone(),
                                logo_path: p.logo_path.clone(),
                                link: data.link.clone(),
                                provider_type: ProviderType::Buy,
                            });
                        }
                    }
                }

                (providers, data.link.clone())
            },
            None => (vec![], None),
        };

        debug!(
            tmdb_id = tmdb_id,
            country = %country,
            provider_count = providers.len(),
            "TMDB API: watch providers result"
        );

        Ok((providers, link))
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<SearchMovie>,
}

#[derive(Debug, Deserialize)]
struct SearchMovie {
    id: i32,
    poster_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MovieDetails {
    poster_path: Option<String>,
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

#[derive(Debug, Deserialize)]
struct WatchProvidersResponse {
    results: std::collections::HashMap<String, WatchProviderCountry>,
}

#[derive(Debug, Deserialize)]
struct WatchProviderCountry {
    link: Option<String>,
    flatrate: Option<Vec<WatchProviderEntry>>,
    rent: Option<Vec<WatchProviderEntry>>,
    buy: Option<Vec<WatchProviderEntry>>,
}

#[derive(Debug, Deserialize)]
struct WatchProviderEntry {
    provider_id: i32,
    provider_name: String,
    logo_path: String,
}
