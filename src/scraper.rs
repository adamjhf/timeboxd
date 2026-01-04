use std::{collections::HashSet, time::Duration};

use scraper::{Html, Selector};

use crate::{error::AppResult, models::WishlistFilm};

pub async fn fetch_watchlist(
    client: &reqwest::Client,
    username: &str,
    delay_ms: u64,
    cutoff_year: i16,
) -> AppResult<Vec<WishlistFilm>> {
    tracing::debug!(username = %username, cutoff_year = cutoff_year, "starting watchlist fetch");

    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let mut page = 1;

    loop {
        let url = if page == 1 {
            format!("https://letterboxd.com/{}/watchlist/by/release/", username)
        } else {
            format!("https://letterboxd.com/{}/watchlist/by/release/page/{}/", username, page)
        };

        tracing::debug!(page = page, url = %url, "fetching watchlist page");
        let html = client.get(&url).send().await?.error_for_status()?.text().await?;
        tracing::debug!(page = page, html_len = html.len(), "fetched HTML");

        let films = parse_watchlist_page(&html)?;
        tracing::debug!(page = page, films_found = films.len(), "parsed films from page");

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
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    tracing::debug!(total_films = out.len(), "completed watchlist fetch");
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
        let title = strip_trailing_year(title);

        tracing::debug!(slug = %slug, title = %title, year = ?year, "found film in watchlist");

        out.push(WishlistFilm { letterboxd_slug: slug.to_string(), year });
    }

    tracing::debug!(film_count = out.len(), "parsed films from page");
    Ok(out)
}

fn strip_trailing_year(title: &str) -> String {
    if let Some((t, y)) = split_trailing_year(title) {
        if y.is_some() {
            return t.trim().to_string();
        }
    }
    title.trim().to_string()
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
    client: &reqwest::Client,
    slug: &str,
) -> AppResult<LetterboxdFilmData> {
    let url = format!("https://letterboxd.com/film/{}/", slug);
    tracing::debug!(slug = %slug, url = %url, "fetching Letterboxd film page");
    let html = client.get(&url).send().await?.error_for_status()?.text().await?;

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
                    tracing::debug!(slug = %slug, tmdb_id = id, "extracted TMDB ID from link");
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

    tracing::debug!(slug = %slug, title = %title, year = ?year, tmdb_id = ?tmdb_id, "parsed Letterboxd film data");

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
