use std::collections::HashMap;

use futures::{StreamExt, stream};
use tracing::{debug, warn};

use crate::{
    cache::{CacheManager, FilmCacheData},
    error::AppResult,
    models::{
        CountryReleases, FilmWithReleases, ReleaseCategory, ReleaseDate, WatchProvider,
        WishlistFilm,
    },
    scraper,
    tmdb::TmdbClient,
};

pub async fn process(
    http: &wreq::Client,
    cache: &CacheManager,
    tmdb: &TmdbClient,
    films: Vec<WishlistFilm>,
    country: &str,
    max_concurrent: usize,
    current_year: i16,
) -> AppResult<Vec<FilmWithReleases>> {
    let cutoff_year = current_year.saturating_sub(3);

    debug!(total_films = films.len(), cutoff_year = cutoff_year, "filtering films by year");

    let films = films
        .into_iter()
        .filter(|f| f.year.map(|y| y >= cutoff_year).unwrap_or(true))
        .collect::<Vec<_>>();

    debug!(filtered_films = films.len(), "films after year filtering");

    if films.is_empty() {
        return Ok(Vec::new());
    }

    // Phase 1: Bulk load film cache
    let slugs: Vec<String> = films.iter().map(|f| f.letterboxd_slug.clone()).collect();
    let cached_films = cache.get_films(&slugs).await?;
    debug!(cached_films = cached_films.len(), "films found in cache");

    // Phase 2: Partition into cached vs uncached
    let (cached, uncached): (Vec<_>, Vec<_>) = films
        .into_iter()
        .partition(|f| cached_films.get(&f.letterboxd_slug).and_then(|c| c.tmdb_id).is_some());

    debug!(cached_count = cached.len(), uncached_count = uncached.len(), "partitioned films");

    // Phase 3: Resolve uncached films (scrape Letterboxd, search TMDB)
    let newly_resolved = resolve_uncached_films(http, tmdb, uncached, max_concurrent).await?;
    cache.upsert_films(newly_resolved.clone()).await?;
    debug!(resolved_count = newly_resolved.len(), "newly resolved films");

    // Phase 4: Build complete film list with TMDB IDs
    let mut all_films_with_tmdb = Vec::new();

    // Add cached films
    for film in cached {
        if let Some(cached_film) = cached_films.get(&film.letterboxd_slug) {
            if let Some(tmdb_id) = cached_film.tmdb_id {
                all_films_with_tmdb.push((
                    film.letterboxd_slug.clone(),
                    tmdb_id,
                    cached_film.title.clone(),
                    cached_film.year.map(|y| y as i16),
                    cached_film.poster_path.clone(),
                ));
            }
        }
    }

    // Add newly resolved films
    for film_data in newly_resolved {
        if let Some(tmdb_id) = film_data.tmdb_id {
            all_films_with_tmdb.push((
                film_data.slug,
                tmdb_id,
                film_data.title,
                film_data.year,
                film_data.poster_path,
            ));
        }
    }

    debug!(total_with_tmdb = all_films_with_tmdb.len(), "films with TMDB IDs");

    // Phase 5: Build list of all (tmdb_id, country) pairs needed
    let release_requests = build_release_requests(&all_films_with_tmdb, country);
    debug!(release_requests = release_requests.len(), "release cache requests");

    // Phase 6: Bulk load release cache
    let cached_releases = cache.get_releases(&release_requests).await?;
    debug!(cached_releases_count = cached_releases.len(), "release sets found in cache");
    for ((tmdb_id, country), (theatrical, streaming)) in &cached_releases {
        debug!(
            tmdb_id = tmdb_id,
            country = %country,
            theatrical_count = theatrical.len(),
            streaming_count = streaming.len(),
            "cached release data"
        );
    }

    // Phase 7: Fetch uncached releases from TMDB
    let uncached_requests: Vec<(i32, String)> =
        release_requests.iter().filter(|req| !cached_releases.contains_key(req)).cloned().collect();
    debug!(uncached_requests_count = uncached_requests.len(), uncached = ?uncached_requests, "uncached requests");

    let mut new_releases = HashMap::new();
    if !uncached_requests.is_empty() {
        debug!(uncached_requests = uncached_requests.len(), "fetching uncached releases from TMDB");

        // Group by tmdb_id to avoid duplicate API calls
        let mut tmdb_ids = HashMap::new();
        for (tmdb_id, country_code) in &uncached_requests {
            tmdb_ids.entry(*tmdb_id).or_insert_with(Vec::new).push(country_code.clone());
        }

        let items: Vec<AppResult<(i32, Vec<String>, Vec<CountryReleases>)>> =
            stream::iter(tmdb_ids)
                .map(|(tmdb_id, countries)| async move {
                    let result = tmdb.get_release_dates(tmdb_id, &countries[0]).await?;
                    let filtered_countries = result
                        .all_countries
                        .into_iter()
                        .filter(|c| countries.contains(&c.country))
                        .collect::<Vec<_>>();
                    Ok((tmdb_id, countries, filtered_countries))
                })
                .buffer_unordered(max_concurrent.max(1))
                .collect()
                .await;

        for item in items {
            match item {
                Ok((tmdb_id, requested_countries, mut found_countries)) => {
                    // Add empty entries for requested countries that had no release data
                    let found_country_codes: Vec<_> =
                        found_countries.iter().map(|c| c.country.clone()).collect();
                    for country_code in requested_countries {
                        if !found_country_codes.contains(&country_code) {
                            found_countries.push(CountryReleases {
                                country: country_code,
                                theatrical: vec![],
                                streaming: vec![],
                            });
                        }
                    }

                    debug!(
                        tmdb_id = tmdb_id,
                        countries = ?found_countries.iter().map(|c| (&c.country, c.theatrical.len(), c.streaming.len())).collect::<Vec<_>>(),
                        "caching release data"
                    );
                    cache.put_releases_multi_country(tmdb_id, &found_countries).await?;
                    new_releases.insert(tmdb_id, found_countries);
                },
                Err(err) => warn!(error = %err, "failed to fetch release dates"),
            }
        }

        debug!(new_releases_cached = new_releases.len(), "new release sets cached");
    }

    // Phase 8: Assemble final results
    let mut results = Vec::new();

    for (slug, tmdb_id, title, year, poster_path) in all_films_with_tmdb {
        debug!(slug = %slug, tmdb_id = tmdb_id, "assembling final result");

        let (theatrical, streaming, category) = get_releases_with_fallback_bulk(
            &cached_releases,
            &new_releases,
            tmdb_id,
            country,
            &slug,
        );

        results.push(FilmWithReleases {
            title,
            year,
            tmdb_id,
            letterboxd_slug: slug,
            poster_path,
            theatrical,
            streaming,
            category,
            streaming_providers: vec![],
        });
    }

    debug!(result_count = results.len(), "completed processing releases");

    let today: jiff::civil::Date = jiff::Zoned::now().into();

    let provider_requests = build_provider_requests(&results, country, &today);
    debug!(provider_requests = provider_requests.len(), "provider cache requests");

    let cached_providers = cache.get_providers(&provider_requests).await?;
    debug!(cached_providers_count = cached_providers.len(), "providers found in cache");

    let uncached_provider_requests: Vec<(i32, String)> = provider_requests
        .iter()
        .filter(|req| !cached_providers.contains_key(req))
        .cloned()
        .collect();
    debug!(
        uncached_provider_requests = uncached_provider_requests.len(),
        "uncached provider requests"
    );

    let mut new_providers: HashMap<(i32, String), Vec<WatchProvider>> = HashMap::new();
    if !uncached_provider_requests.is_empty() {
        let items: Vec<AppResult<(i32, String, Vec<WatchProvider>)>> =
            stream::iter(uncached_provider_requests)
                .map(|(tmdb_id, country_code)| async move {
                    let (providers, _link) =
                        tmdb.get_watch_providers(tmdb_id, &country_code).await?;
                    Ok((tmdb_id, country_code, providers))
                })
                .buffer_unordered(max_concurrent.max(1))
                .collect()
                .await;

        for item in items {
            match item {
                Ok((tmdb_id, country_code, providers)) => {
                    debug!(
                        tmdb_id = tmdb_id,
                        country = %country_code,
                        provider_count = providers.len(),
                        "caching provider data"
                    );
                    cache.put_providers(tmdb_id, &country_code, &providers).await?;
                    new_providers.insert((tmdb_id, country_code), providers);
                },
                Err(err) => warn!(error = %err, "failed to fetch watch providers"),
            }
        }

        debug!(new_providers_cached = new_providers.len(), "new providers cached");
    }

    for result in &mut results {
        let key = (result.tmdb_id, country.to_string());
        if let Some(providers) = cached_providers.get(&key) {
            result.streaming_providers = providers.clone();
        } else if let Some(providers) = new_providers.get(&key) {
            result.streaming_providers = providers.clone();
        }
    }

    debug!(result_count = results.len(), "completed processing");

    results.sort_by_key(|f| f.theatrical.first().or_else(|| f.streaming.first()).map(|r| r.date));

    Ok(results)
}

async fn resolve_uncached_films(
    http: &wreq::Client,
    tmdb: &TmdbClient,
    films: Vec<WishlistFilm>,
    max_concurrent: usize,
) -> AppResult<Vec<FilmCacheData>> {
    debug!(uncached_count = films.len(), "resolving uncached films");

    let items: Vec<AppResult<FilmCacheData>> = stream::iter(films)
        .map(|film| async move {
            debug!(slug = %film.letterboxd_slug, "resolving TMDB ID");

            let (resolved_title, resolved_year, mut tmdb_id, mut poster_path) =
                match scraper::fetch_letterboxd_film_data(http, &film.letterboxd_slug).await {
                    Ok(data) => {
                        if let Some(id) = data.tmdb_id {
                            debug!(slug = %film.letterboxd_slug, tmdb_id = id, "found TMDB ID from Letterboxd");
                        }
                        (data.title, data.year.or(film.year), data.tmdb_id, None)
                    },
                    Err(err) => {
                        warn!(slug = %film.letterboxd_slug, error = %err, "failed to fetch Letterboxd data, using fallback title");
                        let fallback_title = film.letterboxd_slug
                            .split('-')
                            .map(|word| {
                                let mut chars = word.chars();
                                match chars.next() {
                                    None => String::new(),
                                    Some(first) => first.to_uppercase().chain(chars.as_str().chars()).collect()
                                }
                            })
                            .collect::<Vec<String>>()
                            .join(" ");
                        (fallback_title, film.year, None, None)
                    },
                };

            if tmdb_id.is_none() {
                debug!(slug = %film.letterboxd_slug, title = %resolved_title, year = ?resolved_year, "searching TMDB API");
                if let Some((id, poster)) = tmdb.search_movie(&resolved_title, resolved_year).await? {
                    debug!(slug = %film.letterboxd_slug, tmdb_id = id, "found TMDB ID via search");
                    tmdb_id = Some(id);
                    poster_path = poster;
                } else {
                    debug!(slug = %film.letterboxd_slug, "no TMDB ID found");
                }
            } else if poster_path.is_none() {
                poster_path = tmdb.get_movie_details(tmdb_id.unwrap()).await.ok().flatten();
            }

            Ok(FilmCacheData {
                slug: film.letterboxd_slug,
                tmdb_id,
                title: resolved_title,
                year: resolved_year,
                poster_path,
            })
        })
        .buffer_unordered(max_concurrent.max(1))
        .collect()
        .await;

    let mut results = Vec::new();
    for item in items {
        match item {
            Ok(data) => results.push(data),
            Err(err) => warn!(error = %err, "failed to resolve film"),
        }
    }

    Ok(results)
}

fn build_release_requests(
    films: &[(String, i32, String, Option<i16>, Option<String>)],
    country: &str,
) -> Vec<(i32, String)> {
    let mut requests = Vec::new();
    for (_, tmdb_id, _, _, _) in films {
        requests.push((*tmdb_id, country.to_string()));
        if country == "NZ" {
            requests.push((*tmdb_id, "AU".to_string()));
        }
        if country != "US" {
            requests.push((*tmdb_id, "US".to_string()));
        }
    }
    requests
}

fn build_provider_requests(
    films: &[FilmWithReleases],
    country: &str,
    today: &jiff::civil::Date,
) -> Vec<(i32, String)> {
    films
        .iter()
        .filter(|f| needs_provider_lookup(f, today))
        .map(|f| (f.tmdb_id, country.to_string()))
        .collect()
}

fn needs_provider_lookup(film: &FilmWithReleases, today: &jiff::civil::Date) -> bool {
    let has_future_streaming = film.streaming.iter().any(|r| r.date > *today);
    !has_future_streaming
}

fn get_releases_with_fallback_bulk(
    cached_releases: &HashMap<(i32, String), (Vec<ReleaseDate>, Vec<ReleaseDate>)>,
    new_releases: &HashMap<i32, Vec<CountryReleases>>,
    tmdb_id: i32,
    country: &str,
    slug: &str,
) -> (Vec<ReleaseDate>, Vec<ReleaseDate>, ReleaseCategory) {
    let (local_theatrical, local_streaming) =
        get_release_data(cached_releases, new_releases, tmdb_id, country);

    // Separate upcoming releases from "Already available" releases
    let (local_upcoming_theatrical, local_already_available_theatrical): (Vec<_>, Vec<_>) =
        local_theatrical
            .into_iter()
            .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));
    let (local_upcoming_streaming, local_already_available_streaming): (Vec<_>, Vec<_>) =
        local_streaming
            .into_iter()
            .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));

    // Check for recent "Already available" releases first (prioritize over upcoming)
    if !local_already_available_theatrical.is_empty()
        || !local_already_available_streaming.is_empty()
    {
        let mut all_theatrical = local_already_available_theatrical;
        let mut all_streaming = local_already_available_streaming;
        // Mark local releases with country code and include any upcoming releases too
        for rel in &mut all_theatrical {
            rel.note = Some(country.to_string());
        }
        for rel in &mut all_streaming {
            rel.note = Some(country.to_string());
        }
        all_theatrical.extend(local_upcoming_theatrical);
        all_streaming.extend(local_upcoming_streaming);
        return (all_theatrical, all_streaming, ReleaseCategory::LocalAlreadyAvailable);
    }

    // Check for upcoming releases only if no already available releases
    if !local_upcoming_theatrical.is_empty() || !local_upcoming_streaming.is_empty() {
        // Mark local releases with country code
        let mut all_theatrical = local_upcoming_theatrical;
        let mut all_streaming = local_upcoming_streaming;
        for rel in &mut all_theatrical {
            rel.note = Some(country.to_string());
        }
        for rel in &mut all_streaming {
            rel.note = Some(country.to_string());
        }
        return (all_theatrical, all_streaming, ReleaseCategory::LocalUpcoming);
    }

    if country == "US" {
        return (vec![], vec![], ReleaseCategory::NoReleases);
    }

    // Special logic for New Zealand: try Australia first, then US
    if country == "NZ" {
        debug!(slug = %slug, "no NZ releases found, trying Australia");

        let (au_theatrical, au_streaming) =
            get_release_data(cached_releases, new_releases, tmdb_id, "AU");

        if !au_theatrical.is_empty() || !au_streaming.is_empty() {
            // Separate AU releases into upcoming vs already available FIRST
            let (mut au_upcoming_theatrical, mut au_already_available_theatrical): (
                Vec<_>,
                Vec<_>,
            ) = au_theatrical
                .into_iter()
                .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));
            let (mut au_upcoming_streaming, mut au_already_available_streaming): (Vec<_>, Vec<_>) =
                au_streaming.into_iter().partition(|r| {
                    r.note.as_ref().map_or(true, |n| !n.contains("Already available"))
                });

            // Then mark with country code
            for rel in &mut au_upcoming_theatrical {
                rel.note = Some("AU".to_string());
            }
            for rel in &mut au_already_available_theatrical {
                rel.note = Some("AU".to_string());
            }
            for rel in &mut au_upcoming_streaming {
                rel.note = Some("AU".to_string());
            }
            for rel in &mut au_already_available_streaming {
                rel.note = Some("AU".to_string());
            }

            // Put AU releases in appropriate local sections (prioritize already available)
            if !au_already_available_theatrical.is_empty()
                || !au_already_available_streaming.is_empty()
            {
                let mut all_theatrical = au_already_available_theatrical;
                let mut all_streaming = au_already_available_streaming;
                all_theatrical.extend(au_upcoming_theatrical);
                all_streaming.extend(au_upcoming_streaming);
                return (all_theatrical, all_streaming, ReleaseCategory::LocalAlreadyAvailable);
            }

            if !au_upcoming_theatrical.is_empty() || !au_upcoming_streaming.is_empty() {
                return (
                    au_upcoming_theatrical,
                    au_upcoming_streaming,
                    ReleaseCategory::LocalUpcoming,
                );
            }
        }
    }

    // Fall back to US for all non-US countries
    debug!(slug = %slug, "no local releases found, trying US");

    let (us_theatrical, us_streaming) =
        get_release_data(cached_releases, new_releases, tmdb_id, "US");

    if !us_theatrical.is_empty() || !us_streaming.is_empty() {
        let (mut us_upcoming_theatrical, mut us_already_available_theatrical): (Vec<_>, Vec<_>) =
            us_theatrical
                .into_iter()
                .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));
        let (mut us_upcoming_streaming, mut us_already_available_streaming): (Vec<_>, Vec<_>) =
            us_streaming
                .into_iter()
                .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));

        for rel in &mut us_upcoming_theatrical {
            rel.note = Some("US".to_string());
        }
        for rel in &mut us_already_available_theatrical {
            rel.note = Some("US".to_string());
        }
        for rel in &mut us_upcoming_streaming {
            rel.note = Some("US".to_string());
        }
        for rel in &mut us_already_available_streaming {
            rel.note = Some("US".to_string());
        }

        if !us_already_available_theatrical.is_empty() || !us_already_available_streaming.is_empty()
        {
            let mut all_theatrical = us_already_available_theatrical;
            let mut all_streaming = us_already_available_streaming;
            all_theatrical.extend(us_upcoming_theatrical);
            all_streaming.extend(us_upcoming_streaming);
            return (all_theatrical, all_streaming, ReleaseCategory::LocalAlreadyAvailable);
        }

        if !us_upcoming_theatrical.is_empty() || !us_upcoming_streaming.is_empty() {
            return (us_upcoming_theatrical, us_upcoming_streaming, ReleaseCategory::LocalUpcoming);
        }
    }

    (vec![], vec![], ReleaseCategory::NoReleases)
}

fn get_release_data(
    cached_releases: &HashMap<(i32, String), (Vec<ReleaseDate>, Vec<ReleaseDate>)>,
    new_releases: &HashMap<i32, Vec<CountryReleases>>,
    tmdb_id: i32,
    country: &str,
) -> (Vec<ReleaseDate>, Vec<ReleaseDate>) {
    // Try cached data first
    if let Some((theatrical, streaming)) = cached_releases.get(&(tmdb_id, country.to_string())) {
        return (theatrical.clone(), streaming.clone());
    }

    // Try new data
    if let Some(countries) = new_releases.get(&tmdb_id) {
        if let Some(country_data) = countries.iter().find(|c| c.country == country) {
            return (country_data.theatrical.clone(), country_data.streaming.clone());
        }
    }

    (vec![], vec![])
}
