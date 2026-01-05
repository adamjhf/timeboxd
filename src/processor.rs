use futures::{StreamExt, stream};
use tracing::{debug, warn};

use crate::{
    cache::CacheManager,
    error::AppResult,
    models::{FilmWithReleases, ReleaseCategory, WishlistFilm},
    scraper,
    tmdb::TmdbClient,
};

pub async fn process(
    http: &reqwest::Client,
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

    let items: Vec<Option<FilmWithReleases>> = stream::iter(films)
        .map(|film| async move {
            debug!(slug = %film.letterboxd_slug, "processing film");
            let result: AppResult<Option<FilmWithReleases>> = async {
                let Some((tmdb_id, title, year, poster_path)) =
                    resolve_tmdb_id(http, cache, tmdb, &film.letterboxd_slug, film.year).await?
                else {
                    debug!(slug = %film.letterboxd_slug, "no TMDB ID found");
                    return Ok(None);
                };

                debug!(slug = %film.letterboxd_slug, tmdb_id = tmdb_id, "fetching release dates");

                let (theatrical, streaming, category) = get_releases_with_fallback(
                    cache,
                    tmdb,
                    tmdb_id,
                    country,
                    &film.letterboxd_slug,
                )
                .await?;

                let out = FilmWithReleases {
                    title,
                    year,
                    tmdb_id,
                    letterboxd_slug: film.letterboxd_slug.clone(),
                    poster_path,
                    theatrical,
                    streaming,
                    category,
                };

                Ok(Some(out))
            }
            .await;

            match result {
                Ok(film) => film,
                Err(err) => {
                    warn!(slug = %film.letterboxd_slug, error = %err, "failed to process film");
                    None
                },
            }
        })
        .buffer_unordered(max_concurrent.max(1))
        .collect()
        .await;

    let mut results: Vec<FilmWithReleases> = items.into_iter().flatten().collect();

    debug!(result_count = results.len(), "completed processing");

    results.sort_by_key(|f| f.theatrical.first().or_else(|| f.streaming.first()).map(|r| r.date));

    Ok(results)
}

async fn resolve_tmdb_id(
    http: &reqwest::Client,
    cache: &CacheManager,
    tmdb: &TmdbClient,
    slug: &str,
    year: Option<i16>,
) -> AppResult<Option<(i32, String, Option<i16>, Option<String>)>> {
    debug!(slug = %slug, "resolving TMDB ID");

    if let Some(cached) = cache.get_film(slug).await? {
        if let Some(tmdb_id) = cached.tmdb_id {
            debug!(slug = %slug, tmdb_id = tmdb_id, "found cached TMDB ID");
            return Ok(Some((
                tmdb_id,
                cached.title,
                cached.year.map(|y| y as i16),
                cached.poster_path,
            )));
        }
    }

    let (resolved_title, resolved_year, mut tmdb_id, mut poster_path) =
        match scraper::fetch_letterboxd_film_data(http, slug).await {
            Ok(data) => {
                if let Some(id) = data.tmdb_id {
                    debug!(slug = %slug, tmdb_id = id, "found TMDB ID from Letterboxd");
                }
                (data.title, data.year.or(year), data.tmdb_id, None)
            },
            Err(err) => {
                warn!(slug = %slug, error = %err, "failed to fetch Letterboxd data, using fallback title");
                let fallback_title = slug
                    .split('-')
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => {
                                first.to_uppercase().chain(chars.as_str().chars()).collect()
                            },
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(" ");
                (fallback_title, year, None, None)
            },
        };

    if tmdb_id.is_none() {
        debug!(slug = %slug, title = %resolved_title, year = ?resolved_year, "searching TMDB API");
        if let Some((id, poster)) = tmdb.search_movie(&resolved_title, resolved_year).await? {
            debug!(slug = %slug, tmdb_id = id, "found TMDB ID via search");
            tmdb_id = Some(id);
            poster_path = poster;
        } else {
            debug!(slug = %slug, "no TMDB ID found");
        }
    } else if poster_path.is_none() {
        poster_path = tmdb.get_movie_details(tmdb_id.unwrap()).await.ok().flatten();
    }

    cache.upsert_film(slug, tmdb_id, &resolved_title, resolved_year, poster_path.clone()).await?;

    Ok(tmdb_id.map(|id| (id, resolved_title, resolved_year, poster_path)))
}

async fn get_releases_with_fallback(
    cache: &CacheManager,
    tmdb: &TmdbClient,
    tmdb_id: i32,
    country: &str,
    slug: &str,
) -> AppResult<(Vec<crate::models::ReleaseDate>, Vec<crate::models::ReleaseDate>, ReleaseCategory)>
{
    let (local_theatrical, local_streaming) = if let Some(cached) =
        cache.get_releases(tmdb_id, country).await?
    {
        debug!(slug = %slug, "using cached release dates for local country");
        cached
    } else {
        let result = tmdb.get_release_dates(tmdb_id, country).await?;
        let requested = &result.requested_country;
        debug!(slug = %slug, theatrical = requested.theatrical.len(), streaming = requested.streaming.len(), countries = result.all_countries.len(), "fetched release dates");

        let has_mock_data = requested
            .theatrical
            .iter()
            .any(|r| r.note.as_ref().map_or(false, |n| n.contains("Mock")))
            || requested
                .streaming
                .iter()
                .any(|r| r.note.as_ref().map_or(false, |n| n.contains("Mock")));

        if !has_mock_data {
            cache.put_releases_multi_country(tmdb_id, &result.all_countries).await?;
        }

        (requested.theatrical.clone(), requested.streaming.clone())
    };

    // Separate upcoming releases from "Already available" releases
    let (local_upcoming_theatrical, local_already_available_theatrical): (Vec<_>, Vec<_>) =
        local_theatrical
            .into_iter()
            .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));
    let (local_upcoming_streaming, local_already_available_streaming): (Vec<_>, Vec<_>) =
        local_streaming
            .into_iter()
            .partition(|r| r.note.as_ref().map_or(true, |n| !n.contains("Already available")));

    // Check for upcoming releases first
    if !local_upcoming_theatrical.is_empty() || !local_upcoming_streaming.is_empty() {
        let mut all_theatrical = local_upcoming_theatrical;
        let mut all_streaming = local_upcoming_streaming;
        all_theatrical.extend(local_already_available_theatrical);
        all_streaming.extend(local_already_available_streaming);
        return Ok((all_theatrical, all_streaming, ReleaseCategory::LocalUpcoming));
    }

    // Check for recent "Already available" releases
    if !local_already_available_theatrical.is_empty()
        || !local_already_available_streaming.is_empty()
    {
        return Ok((
            local_already_available_theatrical,
            local_already_available_streaming,
            ReleaseCategory::LocalAlreadyAvailable,
        ));
    }

    if country == "US" {
        return Ok((vec![], vec![], ReleaseCategory::NoReleases));
    }

    debug!(slug = %slug, "no local releases found, trying US");

    let (us_theatrical, us_streaming) = if let Some(cached) =
        cache.get_releases(tmdb_id, "US").await?
    {
        debug!(slug = %slug, "using cached US release dates");
        cached
    } else {
        let result = tmdb.get_release_dates(tmdb_id, "US").await?;
        let us_country = result.all_countries.iter().find(|c| c.country == "US");

        if let Some(us) = us_country {
            debug!(slug = %slug, theatrical = us.theatrical.len(), streaming = us.streaming.len(), "fetched US release dates");
            (us.theatrical.clone(), us.streaming.clone())
        } else {
            debug!(slug = %slug, "no US releases found in cached data");
            (vec![], vec![])
        }
    };

    if !us_theatrical.is_empty() || !us_streaming.is_empty() {
        return Ok((us_theatrical, us_streaming, ReleaseCategory::US));
    }

    Ok((vec![], vec![], ReleaseCategory::NoReleases))
}
