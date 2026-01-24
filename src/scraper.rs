use std::{
    collections::HashSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use scraper::{Html, Selector};
use tracing::debug;
use wreq::header::REFERER;

use crate::{error::AppResult, models::WishlistFilm};

pub async fn fetch_watchlist(
    client: &wreq::Client,
    username: &str,
    delay_ms: u64,
    cutoff_year: i16,
) -> AppResult<Vec<WishlistFilm>> {
    debug!(username = %username, cutoff_year = cutoff_year, "fetching watchlist");

    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let mut page = 1;

    loop {
        let url = if page == 1 {
            format!("https://letterboxd.com/{}/watchlist/by/release/", username)
        } else {
            format!("https://letterboxd.com/{}/watchlist/by/release/page/{}/", username, page)
        };

        debug!(page = page, "fetching watchlist page");
        let html = client
            .get(&url)
            .header(REFERER, "https://letterboxd.com/")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let films = parse_watchlist_page(&html)?;
        debug!(page = page, films_found = films.len(), "parsed watchlist page");

        if films.is_empty() {
            break;
        }

        let all_old = films.iter().all(|f| f.year.map(|y| y < cutoff_year).unwrap_or(false));

        for film in films {
            if seen.insert(film.letterboxd_slug.clone()) {
                out.push(film);
            }
        }

        if all_old {
            break;
        }

        page += 1;
        let delay = delay_ms + jitter_ms(150);
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    debug!(username = %username, total_films = out.len(), "completed watchlist fetch");
    Ok(out)
}

fn parse_watchlist_page(html: &str) -> AppResult<Vec<WishlistFilm>> {
    let doc = Html::parse_document(html);
    let selector = Selector::parse("li.griditem div.react-component[data-item-slug]").unwrap();

    let mut out = Vec::new();

    for el in doc.select(&selector) {
        let slug = el.value().attr("data-item-slug");
        let title = el.value().attr("data-item-name");
        let Some(slug) = slug else { continue };
        let Some(title) = title else { continue };

        let year = parse_year_from_title(title);

        out.push(WishlistFilm { letterboxd_slug: slug.to_string(), year });
    }

    Ok(out)
}

fn jitter_ms(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    let nanos =
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos() as u64).unwrap_or(0);
    nanos % (max + 1)
}

fn parse_year_from_title(title: &str) -> Option<i16> {
    split_trailing_year(title).and_then(|(_, y)| y)
}

fn split_trailing_year(title: &str) -> Option<(&str, Option<i16>)> {
    let s = title.trim();
    if !s.ends_with(')') {
        return Some((s, None));
    }
    let open = s.rfind('(')?;
    let inside = &s[open + 1..s.len() - 1];
    if inside.len() != 4 || !inside.chars().all(|c| c.is_ascii_digit()) {
        return Some((s, None));
    }
    let year = inside.parse().ok();
    Some((&s[..open].trim_end(), year))
}

pub struct LetterboxdFilmData {
    pub title: String,
    pub year: Option<i16>,
    pub tmdb_id: Option<i32>,
}

pub async fn fetch_letterboxd_film_data(
    client: &wreq::Client,
    slug: &str,
) -> AppResult<LetterboxdFilmData> {
    let url = format!("https://letterboxd.com/film/{}/", slug);
    debug!(slug = %slug, "fetching Letterboxd film page");
    let html = client
        .get(&url)
        .header(REFERER, "https://letterboxd.com/")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let doc = Html::parse_document(&html);

    let body_selector = Selector::parse("body").unwrap();
    let body = doc.select(&body_selector).next().ok_or_else(|| anyhow::anyhow!("no body tag"))?;

    let mut tmdb_id = body
        .value()
        .attr("data-tmdb-id")
        .filter(|id| !id.is_empty())
        .and_then(|id| id.parse::<i32>().ok());

    if tmdb_id.is_none() {
        let tmdb_link_selector = Selector::parse("a[href*='themoviedb.org']").unwrap();
        if let Some(link) = doc.select(&tmdb_link_selector).next() {
            if let Some(href) = link.value().attr("href") {
                if let Some(id) = extract_tmdb_id_from_url(href) {
                    debug!(slug = %slug, tmdb_id = id, "extracted TMDB ID from link");
                    tmdb_id = Some(id);
                }
            }
        }
    }

    let og_title_selector = Selector::parse("meta[property='og:title']").unwrap();
    let title_with_year = doc
        .select(&og_title_selector)
        .next()
        .and_then(|el| el.value().attr("content"))
        .ok_or_else(|| anyhow::anyhow!("no og:title meta tag"))?;

    let (title, year) = parse_title_and_year(title_with_year);

    debug!(slug = %slug, title = %title, year = ?year, tmdb_id = ?tmdb_id, "parsed Letterboxd film data");

    Ok(LetterboxdFilmData { title: title.to_string(), year, tmdb_id })
}

fn extract_tmdb_id_from_url(url: &str) -> Option<i32> {
    if let Some(movie_pos) = url.find("/movie/") {
        let after_movie = &url[movie_pos + 7..];
        return after_movie.split('/').next().and_then(|id| id.parse().ok());
    }
    if let Some(tv_pos) = url.find("/tv/") {
        let after_tv = &url[tv_pos + 4..];
        return after_tv.split('/').next().and_then(|id| id.parse().ok());
    }
    None
}

fn parse_title_and_year(title_with_year: &str) -> (&str, Option<i16>) {
    let trimmed = title_with_year.trim();
    if let Some((title, year_part)) = split_trailing_year(trimmed) {
        (title.trim(), year_part)
    } else {
        (trimmed, None)
    }
}
