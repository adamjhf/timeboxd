use futures::{StreamExt, stream};

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

    tracing::debug!(
        total_films = films.len(),
        cutoff_year = cutoff_year,
        "filtering films by year"
    );

    let films = films
        .into_iter()
        .filter(|f| f.year.map(|y| y >= cutoff_year).unwrap_or(true))
        .collect::<Vec<_>>();

    tracing::debug!(filtered_films = films.len(), "films after year filtering");

    let items: Vec<Option<FilmWithReleases>> = stream::iter(films)
        .map(|film| async move {
            tracing::debug!(slug = %film.letterboxd_slug, "processing film");
            let result: AppResult<Option<FilmWithReleases>> = async {
                let Some((tmdb_id, title, year)) =
                    resolve_tmdb_id(http, cache, tmdb, &film.letterboxd_slug, film.year).await?
                else {
                    tracing::debug!(slug = %film.letterboxd_slug, "no TMDB ID found, skipping");
                    return Ok(None);
                };

                tracing::debug!(slug = %film.letterboxd_slug, tmdb_id = tmdb_id, "fetching release dates");

                let (theatrical, streaming) =
                    if let Some(cached) = cache.get_releases(tmdb_id, country).await? {
                        tracing::debug!(slug = %film.letterboxd_slug, "using cached release dates");
                        cached
                    } else {
                        let fetched = tmdb.get_release_dates(tmdb_id, country).await?;
                        tracing::debug!(slug = %film.letterboxd_slug, theatrical_count = fetched.0.len(), streaming_count = fetched.1.len(), "fetched release dates");

                        // Don't cache mock release dates (identified by "Mock" in notes)
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
                    theatrical,
                    streaming,
                };

                let has_releases = !out.is_empty();
                tracing::debug!(slug = %film.letterboxd_slug, has_releases = has_releases, "processed film");

                Ok((!out.is_empty()).then_some(out))
            }.await;

            match result {
                Ok(film) => film,
                Err(err) => {
                    tracing::warn!(slug = %film.letterboxd_slug, error = %err, "failed to process film, skipping");
                    None
                }
            }
        })
        .buffer_unordered(max_concurrent.max(1))
        .collect()
        .await;

    let mut results: Vec<FilmWithReleases> = items.into_iter().flatten().collect();

    tracing::debug!(result_count = results.len(), "completed processing");

    results.sort_by_key(|f| f.theatrical.first().or_else(|| f.streaming.first()).map(|r| r.date));

    Ok(results)
}

async fn resolve_tmdb_id(
    http: &reqwest::Client,
    cache: &CacheManager,
    tmdb: &TmdbClient,
    slug: &str,
    year: Option<i16>,
) -> AppResult<Option<(i32, String, Option<i16>)>> {
    tracing::debug!(slug = %slug, "resolving TMDB ID");

    if let Some(cached) = cache.get_film(slug).await? {
        if let Some(tmdb_id) = cached.tmdb_id {
            tracing::debug!(slug = %slug, tmdb_id = tmdb_id, "found cached TMDB ID");
            return Ok(Some((tmdb_id, cached.title, cached.year.map(|y| y as i16))));
        }
    }

    let (resolved_title, resolved_year, mut tmdb_id) = match scraper::fetch_letterboxd_film_json(
        http, slug,
    )
    .await
    {
        Ok(json) => {
            let mut tmdb_id_from_json = None;
            if let Some(links) = json.links {
                for link in links {
                    if link.kind == "tmdb" {
                        if let Some(id) = link.id.and_then(|s| s.parse().ok()) {
                            tmdb_id_from_json = Some(id);
                            tracing::debug!(slug = %slug, tmdb_id = id, "found TMDB ID in Letterboxd JSON");
                            break;
                        }
                    }
                }
            }
            (json.name, json.release_year.or(year), tmdb_id_from_json)
        },
        Err(err) => {
            tracing::debug!(slug = %slug, error = %err, "failed to fetch Letterboxd JSON, will use fallback title");
            // Fallback: use the slug as title (convert kebab-case to title case)
            let fallback_title = slug
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().chain(chars.as_str().chars()).collect(),
                    }
                })
                .collect::<Vec<String>>()
                .join(" ");
            (fallback_title, year, None)
        },
    };

    if tmdb_id.is_none() {
        tracing::debug!(slug = %slug, title = %resolved_title, year = ?resolved_year, "searching TMDB API");
        tmdb_id = tmdb.search_movie(&resolved_title, resolved_year).await?;
        if let Some(id) = tmdb_id {
            tracing::debug!(slug = %slug, tmdb_id = id, "found TMDB ID via search");
        } else {
            tracing::debug!(slug = %slug, "no TMDB ID found");
        }
    }

    cache.upsert_film(slug, tmdb_id, &resolved_title, resolved_year).await?;

    Ok(tmdb_id.map(|id| (id, resolved_title, resolved_year)))
}
