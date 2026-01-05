use futures::{StreamExt, stream};
use tracing::{debug, warn};

use crate::{
    cache::CacheManager,
    error::AppResult,
    models::{FilmWithReleases, WishlistFilm},
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

                let (theatrical, streaming) =
                    if let Some(cached) = cache.get_releases(tmdb_id, country).await? {
                        debug!(slug = %film.letterboxd_slug, "using cached release dates");
                        cached
                    } else {
                        let fetched = tmdb.get_release_dates(tmdb_id, country).await?;
                        debug!(slug = %film.letterboxd_slug, theatrical = fetched.0.len(), streaming = fetched.1.len(), "fetched release dates");

                        let has_mock_data = fetched.0.iter().any(|r| r.note.as_ref().map_or(false, |n| n.contains("Mock")))
                            || fetched.1.iter().any(|r| r.note.as_ref().map_or(false, |n| n.contains("Mock")));

                        if !has_mock_data {
                            cache
                                .put_releases(tmdb_id, country, &fetched.0, &fetched.1)
                                .await?;
                        }

                        fetched
                    };

                let out = FilmWithReleases {
                    title,
                    year,
                    tmdb_id,
                    letterboxd_slug: film.letterboxd_slug.clone(),
                    poster_path,
                    theatrical,
                    streaming,
                };

                Ok((!out.is_empty()).then_some(out))
            }.await;

            match result {
                Ok(film) => film,
                Err(err) => {
                    warn!(slug = %film.letterboxd_slug, error = %err, "failed to process film");
                    None
                }
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
            let poster_path = tmdb.get_movie_details(tmdb_id).await.ok().flatten();
            return Ok(Some((tmdb_id, cached.title, cached.year.map(|y| y as i16), poster_path)));
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

    cache.upsert_film(slug, tmdb_id, &resolved_title, resolved_year).await?;

    Ok(tmdb_id.map(|id| (id, resolved_title, resolved_year, poster_path)))
}
