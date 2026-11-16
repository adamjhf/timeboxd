use std::{
    collections::HashSet,
    io::{Cursor, Read},
};

use anyhow::{Context, anyhow, bail};
use sea_orm::{DatabaseConnection, EntityTrait, Set, sea_query::OnConflict};
use serde::Deserialize;

use crate::{entities::watchlist, models::WishlistFilm};

const MAX_CSV_BYTES: u64 = 10 * 1024 * 1024;
const NOT_AN_EXPORT: &str = "That file doesn't look like a Letterboxd export. Upload the ZIP from \
                             letterboxd.com/user/exportdata, or the watchlist.csv inside it.";

#[derive(Clone, Debug)]
pub struct UploadedWatchlist {
    pub username: Option<String>,
    pub films: Vec<WishlistFilm>,
}

#[derive(Clone, Debug)]
pub struct SavedWatchlist {
    pub token: String,
    pub username: Option<String>,
    pub films: Vec<WishlistFilm>,
    pub uploaded_at: jiff::Timestamp,
}

#[derive(Debug, Deserialize)]
struct WatchlistRow {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Year", deserialize_with = "csv::invalid_option")]
    year: Option<i16>,
    #[serde(rename = "Letterboxd URI")]
    uri: String,
}

/// Parses either the full Letterboxd export ZIP or the `watchlist.csv` from inside it. Errors are
/// written for the person uploading. From the ZIP only `watchlist.csv` and the username column of
/// `profile.csv` are read; everything else, including the email address, is ignored.
pub fn parse_upload(file_name: &str, bytes: &[u8]) -> anyhow::Result<UploadedWatchlist> {
    if bytes.starts_with(b"PK\x03\x04") {
        let mut archive =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| anyhow!(NOT_AN_EXPORT))?;
        let csv = read_zip_entry(&mut archive, "watchlist.csv")?.ok_or_else(|| {
            anyhow!(
                "This ZIP doesn't contain watchlist.csv. Make sure it's the export from \
                 letterboxd.com/user/exportdata."
            )
        })?;
        let username =
            read_zip_entry(&mut archive, "profile.csv")?.and_then(|p| parse_username(&p));
        return Ok(UploadedWatchlist { username, films: parse_watchlist_csv(&csv)? });
    }

    let base_name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name).to_lowercase();
    if matches!(base_name.as_str(), "watched.csv" | "ratings.csv" | "diary.csv" | "reviews.csv") {
        bail!(
            "That's {base_name}, which lists films you've already logged. Upload watchlist.csv \
             from the same export, or the whole ZIP."
        );
    }

    let text = std::str::from_utf8(bytes).map_err(|_| anyhow!(NOT_AN_EXPORT))?;
    Ok(UploadedWatchlist { username: None, films: parse_watchlist_csv(text)? })
}

fn read_zip_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> anyhow::Result<Option<String>> {
    let nested = format!("/{name}");
    let Some(path) = archive
        .file_names()
        .filter_map(Result::ok)
        .filter(|path| *path == name || path.ends_with(&nested))
        .min_by_key(|path| path.matches('/').count())
        .map(|path| path.into_owned())
    else {
        return Ok(None);
    };

    let entry = archive.by_name(&path).map_err(|_| anyhow!(NOT_AN_EXPORT))?;
    let mut text = String::new();
    entry.take(MAX_CSV_BYTES).read_to_string(&mut text).map_err(|_| anyhow!(NOT_AN_EXPORT))?;
    Ok(Some(text))
}

fn parse_watchlist_csv(text: &str) -> anyhow::Result<Vec<WishlistFilm>> {
    let not_watchlist = || {
        anyhow!(
            "That file doesn't look like watchlist.csv from a Letterboxd export. Upload the ZIP \
             from letterboxd.com/user/exportdata, or the watchlist.csv inside it."
        )
    };

    let mut reader = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    let headers = reader.headers().map_err(|_| not_watchlist())?;
    if !["Name", "Year", "Letterboxd URI"].iter().all(|h| headers.iter().any(|c| c == *h)) {
        return Err(not_watchlist());
    }

    let mut seen = HashSet::new();
    let mut films = Vec::new();
    for row in reader.deserialize::<WatchlistRow>() {
        let row = row.map_err(|_| not_watchlist())?;
        let (title, uri) = (row.name.trim(), row.uri.trim());
        if title.is_empty() || uri.is_empty() || !seen.insert(uri.to_string()) {
            continue;
        }
        films.push(WishlistFilm {
            letterboxd_uri: uri.to_string(),
            title: title.to_string(),
            year: row.year,
        });
    }

    Ok(films)
}

fn parse_username(profile_csv: &str) -> Option<String> {
    let mut reader =
        csv::Reader::from_reader(profile_csv.trim_start_matches('\u{feff}').as_bytes());
    let column = reader.headers().ok()?.iter().position(|h| h == "Username")?;
    let record = reader.records().next()?.ok()?;
    record.get(column).map(str::trim).filter(|u| !u.is_empty()).map(str::to_string)
}

/// Stores an upload, replacing the watchlist saved under `token` so existing links keep working.
pub async fn save(
    db: &DatabaseConnection,
    token: Option<&str>,
    upload: UploadedWatchlist,
) -> anyhow::Result<String> {
    let token = token.map_or_else(|| uuid::Uuid::new_v4().simple().to_string(), str::to_string);
    let model = watchlist::ActiveModel {
        token: Set(token.clone()),
        username: Set(upload.username),
        films: Set(serde_json::to_string(&upload.films)?),
        uploaded_at: Set(jiff::Timestamp::now().as_second()),
    };

    watchlist::Entity::insert(model)
        .on_conflict(
            OnConflict::column(watchlist::Column::Token)
                .update_columns([
                    watchlist::Column::Username,
                    watchlist::Column::Films,
                    watchlist::Column::UploadedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await
        .context("failed to save watchlist")?;

    Ok(token)
}

pub async fn load(db: &DatabaseConnection, token: &str) -> anyhow::Result<Option<SavedWatchlist>> {
    let Some(row) = watchlist::Entity::find_by_id(token).one(db).await? else {
        return Ok(None);
    };

    Ok(Some(SavedWatchlist {
        token: row.token,
        username: row.username,
        films: serde_json::from_str(&row.films).context("stored watchlist is corrupt")?,
        uploaded_at: jiff::Timestamp::from_second(row.uploaded_at)?,
    }))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const WATCHLIST: &str = "Date,Name,Year,Letterboxd URI\n\
        2024-01-02,\"The Good, the Bad and the Ugly\",1966,https://boxd.it/2ape\n\
        2025-03-04,Untitled Project,,https://boxd.it/abc1\n";

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, body) in files {
            writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_watchlist_and_username_from_export_zip() {
        let zip = zip_of(&[
            ("profile.csv", "Date Joined,Username,Email Address\n2014-01-01,freeth,a@b.c\n"),
            (
                "lists/sophia-watchlist.csv",
                "Letterboxd list export v7\nDate,Name,Tags,URL,Description\n",
            ),
            ("watchlist.csv", WATCHLIST),
        ]);

        let upload = parse_upload("letterboxd-freeth.zip", &zip).unwrap();

        assert_eq!(upload.username.as_deref(), Some("freeth"));
        let films: Vec<_> = upload
            .films
            .iter()
            .map(|f| (f.title.as_str(), f.year, f.letterboxd_uri.as_str()))
            .collect();
        assert_eq!(
            films,
            [
                ("The Good, the Bad and the Ugly", Some(1966), "https://boxd.it/2ape"),
                ("Untitled Project", None, "https://boxd.it/abc1"),
            ]
        );
    }

    #[test]
    fn reads_bare_watchlist_csv() {
        let upload =
            parse_upload("watchlist.csv", format!("\u{feff}{WATCHLIST}").as_bytes()).unwrap();
        assert_eq!(upload.username, None);
        assert_eq!(upload.films.len(), 2);
    }

    #[test]
    fn rejects_other_files() {
        let watched = parse_upload("watched.csv", WATCHLIST.as_bytes()).unwrap_err();
        assert!(watched.to_string().contains("Upload watchlist.csv"));

        let zip = zip_of(&[("profile.csv", "Username\nfreeth\n")]);
        let missing = parse_upload("export.zip", &zip).unwrap_err();
        assert!(missing.to_string().contains("doesn't contain watchlist.csv"));

        let list = "Letterboxd list export v7\nDate,Name,Tags,URL,Description\n";
        assert!(parse_upload("my-list.csv", list.as_bytes()).is_err());
    }
}
