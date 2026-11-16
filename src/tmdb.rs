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
    client: reqwest::Client,
    access_token: String,
    base_url: String,
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl TmdbClient {
    pub fn new(client: reqwest::Client, access_token: String, base_url: String, rps: u32) -> Self {
        if access_token.trim().is_empty() {
            warn!("TMDB_ACCESS_TOKEN not provided, using mock data");
        }

        let limiter =
            Arc::new(RateLimiter::direct(Quota::per_second(NonZeroU32::new(rps.max(1)).unwrap())));
        Self { client, access_token, base_url, limiter }
    }

    /// Finds the TMDB movie for a Letterboxd title and year. Only exact title matches count, so a
    /// film that is not on TMDB stays unmatched instead of resolving to a different film.
    pub async fn search_movie(
        &self,
        title: &str,
        year: Option<i16>,
    ) -> AppResult<Option<(i32, Option<String>)>> {
        if self.access_token.trim().is_empty() {
            return Ok(Some((550, None)));
        }

        let results = self.search(title, year).await?;
        let mut found = find_match(&results, title, year, year.is_some()).cloned();
        if found.is_none() && year.is_some() {
            let results = self.search(title, None).await?;
            found = find_match(&results, title, year, false).cloned();
        }

        debug!(title = %title, year = ?year, tmdb_id = ?found.as_ref().map(|m| m.id), "TMDB API: search match");
        Ok(found.map(|m| (m.id, m.poster_path)))
    }

    async fn search(&self, title: &str, year: Option<i16>) -> AppResult<Vec<SearchMovie>> {
        self.limiter.until_ready().await;

        debug!(title = %title, year = ?year, "TMDB API: searching movie");

        let url = format!("{}/search/movie", self.base_url.trim_end_matches('/'));
        let year = year.map(|y| y.to_string());
        let mut params = vec![("query", title)];
        params.extend(year.as_deref().map(|y| ("year", y)));

        let resp: SearchResponse = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.results)
    }

    pub async fn get_release_dates(
        &self,
        tmdb_id: i32,
        country: &str,
    ) -> AppResult<ReleaseDatesResult> {
        // Use mock data if access token is not provided
        if self.access_token.trim().is_empty() {
            let today: Date =
                jiff::Timestamp::now().to_zoned_datetime(jiff::tz::TimeZone::UTC).date();
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

        let today: Date = jiff::ZonedDateTime::now().into();

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
                let date: Date = timestamp.to_zoned_datetime(jiff::tz::TimeZone::UTC).date();
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

            if has_past_theatrical
                && theatrical.is_empty()
                && let Some(latest) = theatrical_past.into_iter().max_by_key(|r| r.date)
                && latest.date >= two_years_ago
            {
                theatrical.push(ReleaseDate {
                    date: latest.date,
                    release_type: ReleaseType::Theatrical,
                    note: Some("Already available".to_string()),
                });
            }

            if has_past_streaming
                && streaming.is_empty()
                && let Some(latest) = streaming_past.into_iter().max_by_key(|r| r.date)
                && latest.date >= two_years_ago
            {
                streaming.push(ReleaseDate {
                    date: latest.date,
                    release_type: ReleaseType::Digital,
                    note: Some("Already available".to_string()),
                });
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

#[derive(Clone, Debug, Deserialize)]
struct SearchMovie {
    id: i32,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    poster_path: Option<String>,
}

impl SearchMovie {
    fn has_title(&self, wanted: &str) -> bool {
        normalize_title(&self.title) == wanted
            || self.original_title.as_deref().is_some_and(|t| normalize_title(t) == wanted)
    }

    /// Letterboxd takes a film's year from its TMDB release dates, so a film without a year on
    /// Letterboxd has no release date on TMDB either.
    fn is_near_year(&self, year: Option<i16>) -> bool {
        let release_year = self
            .release_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse::<i16>().ok());
        match (year, release_year) {
            (Some(year), Some(release_year)) => (release_year - year).abs() <= 1,
            (None, None) => true,
            _ => false,
        }
    }
}

/// Picks the first exact-title result whose primary release year is within a year of the
/// Letterboxd year (or that has no release date when Letterboxd has no year), keeping TMDB's
/// ranking. TMDB's `year` filter also matches re-releases, so a
/// 2024 search for a remake can rank the 1994 original first. Results from a year-filtered
/// search all have a release date in that year, so `accept_any_year` lets them through when no
/// primary year is close, as with festival films whose premiere year differs.
fn find_match<'a>(
    results: &'a [SearchMovie],
    title: &str,
    year: Option<i16>,
    accept_any_year: bool,
) -> Option<&'a SearchMovie> {
    let wanted = normalize_title(title);
    let mut exact = results.iter().filter(|r| r.has_title(&wanted));
    let first_exact = exact.clone().next();
    exact.find(|r| r.is_near_year(year)).or(first_exact.filter(|_| accept_any_year))
}

fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .replace('&', " and ")
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn movie(id: i32, title: &str, release_date: &str) -> SearchMovie {
        SearchMovie {
            id,
            title: title.to_string(),
            original_title: None,
            release_date: Some(release_date.to_string()),
            poster_path: None,
        }
    }

    #[test]
    fn prefers_remake_over_higher_ranked_original() {
        let results =
            [movie(9495, "The Crow", "1994-05-11"), movie(957452, "The Crow", "2024-08-21")];
        assert_eq!(find_match(&results, "The Crow", Some(2024), true).map(|m| m.id), Some(957452));
    }

    #[test]
    fn never_matches_a_different_title() {
        let results = [movie(1, "Life of Brian", "1979-08-17")];
        assert!(find_match(&results, "Brian", Some(2026), true).is_none());
    }

    #[test]
    fn year_filtered_results_accept_distant_primary_year() {
        let results = [movie(1486475, "No Rest for the Wicked", "2026-09-03")];
        let title = "No Rest for the Wicked";
        assert_eq!(find_match(&results, title, Some(2024), true).map(|m| m.id), Some(1486475));
        assert!(find_match(&results, title, Some(2024), false).is_none());
    }

    #[test]
    fn film_without_year_only_matches_undated_results() {
        let results = [movie(8009, "Highlander", "1986-03-07"), movie(339727, "Highlander", "")];
        assert_eq!(find_match(&results, "Highlander", None, false).map(|m| m.id), Some(339727));

        let results = [movie(8095, "Cleopatra", "1963-06-12")];
        assert!(find_match(&results, "Cleopatra", None, false).is_none());
    }

    #[test]
    fn matches_original_title_and_ignores_punctuation() {
        let mut film = movie(7, "The Good, the Bad & the Ugly", "1966-12-23");
        film.original_title = Some("Il buono, il brutto, il cattivo".to_string());
        let results = [film];
        assert!(find_match(&results, "The Good, the Bad and the Ugly", Some(1966), true).is_some());
        assert!(
            find_match(&results, "Il Buono, il Brutto, il Cattivo", Some(1966), true).is_some()
        );
    }
}
