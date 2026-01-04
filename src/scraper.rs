use std::{collections::HashSet, time::Duration};

use scraper::{Html, Selector};
use serde::Deserialize;

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

        out.push(WishlistFilm { letterboxd_slug: slug.to_string(), title, year, tmdb_id: None });
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
    Some((&s[..open], year))
}

#[derive(Debug, Deserialize)]
pub struct LetterboxdFilmJson {
    pub name: String,
    #[serde(rename = "releaseYear")]
    pub release_year: Option<i16>,
    pub links: Option<Vec<LetterboxdFilmJsonLink>>,
}

#[derive(Debug, Deserialize)]
pub struct LetterboxdFilmJsonLink {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Option<String>,
}

pub async fn fetch_letterboxd_film_json(
    client: &reqwest::Client,
    slug: &str,
) -> AppResult<LetterboxdFilmJson> {
    let url = format!("https://letterboxd.com/film/{}/json/", slug);
    tracing::debug!(slug = %slug, url = %url, "fetching Letterboxd JSON");
    let resp = client.get(&url).send().await?.error_for_status()?;

    if let Some(content_type) = resp.headers().get("content-type") {
        if let Ok(ct) = content_type.to_str() {
            if !ct.contains("application/json")
                && !ct.contains("text/json")
                && !ct.contains("+json")
            {
                tracing::debug!(slug = %slug, content_type = %ct, "not JSON response, skipping");
                return Err(anyhow::anyhow!("not JSON response").into());
            }
        }
    }

    let text = resp.text().await?;

    if text.trim_start().starts_with('<') {
        tracing::debug!(slug = %slug, "response appears to be HTML, not JSON");
        return Err(anyhow::anyhow!("HTML response instead of JSON").into());
    }

    let json =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("failed to parse JSON: {}", e))?;
    tracing::debug!(slug = %slug, "successfully fetched Letterboxd JSON");
    Ok(json)
}
