use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use tracing::debug;

use crate::{
    entities::{
        film_cache, provider_cache, provider_cache_meta, release_cache, release_cache_meta,
    },
    error::AppResult,
    models::{ProviderType, ReleaseDate, ReleaseType, WatchProvider},
};

#[derive(Clone, Debug)]
pub struct FilmCacheData {
    pub slug: String,
    pub tmdb_id: Option<i32>,
    pub title: String,
    pub year: Option<i16>,
    pub poster_path: Option<String>,
}

#[derive(Clone)]
pub struct CacheManager {
    db: DatabaseConnection,
    film_ttl_seconds: i64,
    release_ttl_seconds: i64,
    provider_ttl_seconds: i64,
}

impl CacheManager {
    pub fn new(
        db: DatabaseConnection,
        film_ttl_days: i64,
        release_ttl_hours: i64,
        provider_ttl_days: i64,
    ) -> Self {
        Self {
            db,
            film_ttl_seconds: film_ttl_days * 86_400,
            release_ttl_seconds: release_ttl_hours * 3_600,
            provider_ttl_seconds: provider_ttl_days * 86_400,
        }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn get_films(
        &self,
        slugs: &[String],
    ) -> AppResult<HashMap<String, film_cache::Model>> {
        if slugs.is_empty() {
            return Ok(HashMap::new());
        }

        let films = film_cache::Entity::find()
            .filter(film_cache::Column::LetterboxdSlug.is_in(slugs.iter().cloned()))
            .all(&self.db)
            .await?;

        let mut result = HashMap::new();
        for film in films {
            if self.is_film_fresh(film.updated_at) {
                result.insert(film.letterboxd_slug.clone(), film);
            }
        }

        Ok(result)
    }

    pub async fn upsert_films(&self, films: Vec<FilmCacheData>) -> AppResult<()> {
        if films.is_empty() {
            return Ok(());
        }

        let now = now_sec();
        let txn = self.db.begin().await?;

        for film in films {
            let model = film_cache::ActiveModel {
                letterboxd_slug: Set(film.slug),
                tmdb_id: Set(film.tmdb_id),
                title: Set(film.title),
                year: Set(film.year.map(|y| y as i32)),
                poster_path: Set(film.poster_path),
                updated_at: Set(now),
            };

            film_cache::Entity::insert(model)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(film_cache::Column::LetterboxdSlug)
                        .update_columns([
                            film_cache::Column::TmdbId,
                            film_cache::Column::Title,
                            film_cache::Column::Year,
                            film_cache::Column::PosterPath,
                            film_cache::Column::UpdatedAt,
                        ])
                        .to_owned(),
                )
                .exec(&txn)
                .await?;
        }

        txn.commit().await?;

        Ok(())
    }

    pub async fn get_releases(
        &self,
        requests: &[(i32, String)],
    ) -> AppResult<HashMap<(i32, String), (Vec<ReleaseDate>, Vec<ReleaseDate>)>> {
        if requests.is_empty() {
            return Ok(HashMap::new());
        }

        let request_set: HashSet<(i32, String)> = requests.iter().cloned().collect();
        let tmdb_ids: Vec<i32> = requests.iter().map(|(id, _)| *id).collect();

        debug!(
            request_count = requests.len(),
            tmdb_id_count = tmdb_ids.len(),
            "cache lookup: starting"
        );

        // Query meta table for all tmdb_ids we're interested in
        let metas = release_cache_meta::Entity::find()
            .filter(release_cache_meta::Column::TmdbId.is_in(tmdb_ids.clone()))
            .all(&self.db)
            .await?;

        debug!(meta_count = metas.len(), "cache lookup: found meta entries");

        // Filter to only fresh meta entries that match our requested (tmdb_id, country) pairs
        let fresh_requests: Vec<(i32, String)> = metas
            .into_iter()
            .filter(|meta| {
                let is_fresh = self.is_release_fresh(meta.cached_at);
                let in_request = request_set.contains(&(meta.tmdb_id, meta.country.clone()));
                debug!(
                    tmdb_id = meta.tmdb_id,
                    country = %meta.country,
                    is_fresh = is_fresh,
                    in_request = in_request,
                    "cache lookup: checking meta"
                );
                is_fresh && in_request
            })
            .map(|meta| (meta.tmdb_id, meta.country))
            .collect();

        debug!(fresh_count = fresh_requests.len(), "cache lookup: fresh requests");

        if fresh_requests.is_empty() {
            return Ok(HashMap::new());
        }

        let fresh_tmdb_ids: Vec<i32> = fresh_requests.iter().map(|(id, _)| *id).collect();
        let fresh_set: HashSet<(i32, String)> = fresh_requests.iter().cloned().collect();

        // Query all release data for fresh tmdb_ids
        let rows = release_cache::Entity::find()
            .filter(release_cache::Column::TmdbId.is_in(fresh_tmdb_ids))
            .all(&self.db)
            .await?;

        // Group rows by (tmdb_id, country), filtering to only requested pairs
        let mut grouped: HashMap<(i32, String), Vec<_>> = HashMap::new();
        for row in rows {
            let key = (row.tmdb_id, row.country.clone());
            if fresh_set.contains(&key) {
                grouped.entry(key).or_default().push(row);
            }
        }

        let mut result = HashMap::new();

        // Include all fresh requests in result, even if they have no release rows
        for key in fresh_requests {
            let rows = grouped.remove(&key).unwrap_or_default();
            let mut theatrical = Vec::new();
            let mut streaming = Vec::new();

            for row in rows {
                let Ok(date) = row.release_date.parse() else {
                    continue;
                };
                let Some(kind) = ReleaseType::from_tmdb_code(row.release_type) else {
                    continue;
                };
                let rd = ReleaseDate { date, release_type: kind, note: row.note };
                match kind {
                    ReleaseType::Theatrical => theatrical.push(rd),
                    ReleaseType::Digital => streaming.push(rd),
                }
            }

            theatrical.sort_by_key(|r| r.date);
            streaming.sort_by_key(|r| r.date);

            result.insert(key, (theatrical, streaming));
        }

        Ok(result)
    }

    pub async fn put_releases(
        &self,
        tmdb_id: i32,
        country: &str,
        theatrical: &[ReleaseDate],
        streaming: &[ReleaseDate],
    ) -> AppResult<()> {
        let now = now_sec();

        let txn = self.db.begin().await?;

        release_cache::Entity::delete_many()
            .filter(release_cache::Column::TmdbId.eq(tmdb_id))
            .filter(release_cache::Column::Country.eq(country))
            .exec(&txn)
            .await?;

        for rel in theatrical.iter().chain(streaming.iter()) {
            let model = release_cache::ActiveModel {
                id: Default::default(),
                tmdb_id: Set(tmdb_id),
                country: Set(country.to_string()),
                release_date: Set(rel.date.to_string()),
                release_type: Set(rel.release_type.as_tmdb_code()),
                note: Set(rel.note.clone()),
                cached_at: Set(now),
            };
            release_cache::Entity::insert(model).exec(&txn).await?;
        }

        let meta = release_cache_meta::ActiveModel {
            id: Default::default(),
            tmdb_id: Set(tmdb_id),
            country: Set(country.to_string()),
            cached_at: Set(now),
        };

        release_cache_meta::Entity::insert(meta)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    release_cache_meta::Column::TmdbId,
                    release_cache_meta::Column::Country,
                ])
                .update_columns([release_cache_meta::Column::CachedAt])
                .to_owned(),
            )
            .exec(&txn)
            .await?;

        txn.commit().await?;

        Ok(())
    }

    pub async fn put_releases_multi_country(
        &self,
        tmdb_id: i32,
        countries: &[crate::models::CountryReleases],
    ) -> AppResult<()> {
        let now = now_sec();
        let country_codes: Vec<String> = countries.iter().map(|c| c.country.clone()).collect();

        let txn = self.db.begin().await?;

        // Only delete release data for the specific countries we're updating
        release_cache::Entity::delete_many()
            .filter(release_cache::Column::TmdbId.eq(tmdb_id))
            .filter(release_cache::Column::Country.is_in(country_codes))
            .exec(&txn)
            .await?;

        for country_data in countries {
            for rel in country_data.theatrical.iter().chain(country_data.streaming.iter()) {
                let model = release_cache::ActiveModel {
                    id: Default::default(),
                    tmdb_id: Set(tmdb_id),
                    country: Set(country_data.country.clone()),
                    release_date: Set(rel.date.to_string()),
                    release_type: Set(rel.release_type.as_tmdb_code()),
                    note: Set(rel.note.clone()),
                    cached_at: Set(now),
                };
                release_cache::Entity::insert(model).exec(&txn).await?;
            }

            let meta = release_cache_meta::ActiveModel {
                id: Default::default(),
                tmdb_id: Set(tmdb_id),
                country: Set(country_data.country.clone()),
                cached_at: Set(now),
            };

            release_cache_meta::Entity::insert(meta)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::columns([
                        release_cache_meta::Column::TmdbId,
                        release_cache_meta::Column::Country,
                    ])
                    .update_columns([release_cache_meta::Column::CachedAt])
                    .to_owned(),
                )
                .exec(&txn)
                .await?;
        }

        txn.commit().await?;

        Ok(())
    }

    pub async fn clear_mock_release_dates(&self) -> AppResult<()> {
        release_cache::Entity::delete_many()
            .filter(release_cache::Column::Note.contains("Mock"))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    pub async fn get_providers(
        &self,
        requests: &[(i32, String)],
    ) -> AppResult<HashMap<(i32, String), Vec<WatchProvider>>> {
        if requests.is_empty() {
            return Ok(HashMap::new());
        }

        let request_set: HashSet<(i32, String)> = requests.iter().cloned().collect();
        let tmdb_ids: Vec<i32> = requests.iter().map(|(id, _)| *id).collect();

        debug!(
            request_count = requests.len(),
            tmdb_id_count = tmdb_ids.len(),
            "provider cache lookup: starting"
        );

        let metas = provider_cache_meta::Entity::find()
            .filter(provider_cache_meta::Column::TmdbId.is_in(tmdb_ids.clone()))
            .all(&self.db)
            .await?;

        debug!(meta_count = metas.len(), "provider cache lookup: found meta entries");

        let fresh_requests: Vec<(i32, String)> = metas
            .into_iter()
            .filter(|meta| {
                let is_fresh = self.is_provider_fresh(meta.cached_at);
                let in_request = request_set.contains(&(meta.tmdb_id, meta.country.clone()));
                is_fresh && in_request
            })
            .map(|meta| (meta.tmdb_id, meta.country))
            .collect();

        debug!(fresh_count = fresh_requests.len(), "provider cache lookup: fresh requests");

        if fresh_requests.is_empty() {
            return Ok(HashMap::new());
        }

        let fresh_tmdb_ids: Vec<i32> = fresh_requests.iter().map(|(id, _)| *id).collect();
        let fresh_set: HashSet<(i32, String)> = fresh_requests.iter().cloned().collect();

        let rows = provider_cache::Entity::find()
            .filter(provider_cache::Column::TmdbId.is_in(fresh_tmdb_ids))
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<(i32, String), Vec<_>> = HashMap::new();
        for row in rows {
            let key = (row.tmdb_id, row.country.clone());
            if fresh_set.contains(&key) {
                grouped.entry(key).or_default().push(row);
            }
        }

        let mut result = HashMap::new();

        for key in fresh_requests {
            let rows = grouped.remove(&key).unwrap_or_default();
            let providers: Vec<WatchProvider> = rows
                .into_iter()
                .filter_map(|row| {
                    Some(WatchProvider {
                        provider_id: row.provider_id,
                        provider_name: row.provider_name,
                        logo_path: row.logo_path,
                        link: row.link,
                        provider_type: ProviderType::from_code(row.provider_type)?,
                    })
                })
                .collect();
            result.insert(key, providers);
        }

        Ok(result)
    }

    pub async fn put_providers(
        &self,
        tmdb_id: i32,
        country: &str,
        providers: &[WatchProvider],
    ) -> AppResult<()> {
        if providers.is_empty() {
            return Ok(());
        }

        let now = now_sec();
        let txn = self.db.begin().await?;

        for provider in providers {
            let model = provider_cache::ActiveModel {
                id: Default::default(),
                tmdb_id: Set(tmdb_id),
                country: Set(country.to_string()),
                provider_id: Set(provider.provider_id),
                provider_name: Set(provider.provider_name.clone()),
                logo_path: Set(provider.logo_path.clone()),
                link: Set(provider.link.clone()),
                provider_type: Set(provider.provider_type.as_code()),
                cached_at: Set(now),
            };
            provider_cache::Entity::insert(model)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::columns([
                        provider_cache::Column::TmdbId,
                        provider_cache::Column::Country,
                        provider_cache::Column::ProviderId,
                        provider_cache::Column::ProviderType,
                    ])
                    .update_columns([
                        provider_cache::Column::ProviderName,
                        provider_cache::Column::LogoPath,
                        provider_cache::Column::Link,
                        provider_cache::Column::CachedAt,
                    ])
                    .to_owned(),
                )
                .exec(&txn)
                .await?;
        }

        let meta = provider_cache_meta::ActiveModel {
            id: Default::default(),
            tmdb_id: Set(tmdb_id),
            country: Set(country.to_string()),
            cached_at: Set(now),
        };

        provider_cache_meta::Entity::insert(meta)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    provider_cache_meta::Column::TmdbId,
                    provider_cache_meta::Column::Country,
                ])
                .update_columns([provider_cache_meta::Column::CachedAt])
                .to_owned(),
            )
            .exec(&txn)
            .await?;

        txn.commit().await?;

        Ok(())
    }

    fn is_film_fresh(&self, cached_at: i64) -> bool {
        now_sec().saturating_sub(cached_at) <= self.film_ttl_seconds
    }

    fn is_release_fresh(&self, cached_at: i64) -> bool {
        now_sec().saturating_sub(cached_at) <= self.release_ttl_seconds
    }

    fn is_provider_fresh(&self, cached_at: i64) -> bool {
        now_sec().saturating_sub(cached_at) <= self.provider_ttl_seconds
    }
}

fn now_sec() -> i64 {
    jiff::Timestamp::now().as_second()
}
