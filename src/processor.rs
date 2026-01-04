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

    let films = films
        .into_iter()
        .filter(|f| f.year.map(|y| y >= cutoff_year).unwrap_or(true))
        .collect::<Vec<_>>();

    let items: Vec<AppResult<Option<FilmWithReleases>>> = stream::iter(films)
        .map(|film| async move {
            let Some((tmdb_id, title, year)) =
                resolve_tmdb_id(http, cache, tmdb, &film.letterboxd_slug, film.year).await?
            else {
                return Ok(None);
            };

            let (theatrical, streaming) =
                if let Some(cached) = cache.get_releases(tmdb_id, country).await? {
                    cached
                } else {
                    let fetched = tmdb.get_release_dates(tmdb_id, country).await?;
                    cache
                        .put_releases(tmdb_id, country, &fetched.0, &fetched.1)
                        .await?;
                    fetched
                };

            let out = FilmWithReleases {
                title,
                year,
                tmdb_id,
                theatrical,
                streaming,
            };

            Ok((!out.is_empty()).then_some(out))
        })
        .buffer_unordered(max_concurrent.max(1))
        .collect()
        .await;

    let mut results = Vec::new();
    for item in items {
        if let Some(film) = item? {
            results.push(film);
        }
    }

    results.sort_by_key(|f| {
        f.theatrical
            .first()
            .or_else(|| f.streaming.first())
            .map(|r| r.date)
    });

    Ok(results)
}

async fn resolve_tmdb_id(
    http: &reqwest::Client,
    cache: &CacheManager,
    tmdb: &TmdbClient,
    slug: &str,
    year: Option<i16>,
) -> AppResult<Option<(i32, String, Option<i16>)>> {
    if let Some(cached) = cache.get_film(slug).await? {
        if let Some(tmdb_id) = cached.tmdb_id {
            return Ok(Some((tmdb_id, cached.title, cached.year.map(|y| y as i16))));
        }
    }

    let json = scraper::fetch_letterboxd_film_json(http, slug).await?;
    let mut tmdb_id = None;
    for link in json.film.links {
        if link.kind == "tmdb" {
            if let Some(id) = link.id.and_then(|s| s.parse().ok()) {
                tmdb_id = Some(id);
                break;
            }
        }
    }

    let resolved_title = json.film.name;
    let resolved_year = json.film.release_year.or(year);

    if tmdb_id.is_none() {
        tmdb_id = tmdb.search_movie(&resolved_title, resolved_year).await?;
    }

    cache
        .upsert_film(slug, tmdb_id, &resolved_title, resolved_year)
        .await?;

    Ok(tmdb_id.map(|id| (id, resolved_title, resolved_year)))
}
