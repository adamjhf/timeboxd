use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};

use crate::{
    entities::{film_cache, release_cache, release_cache_meta},
    error::AppResult,
    models::{ReleaseDate, ReleaseType},
};

#[derive(Clone)]
pub struct CacheManager {
    db: DatabaseConnection,
    ttl_seconds: i64,
}

impl CacheManager {
    pub fn new(db: DatabaseConnection, ttl_days: i64) -> Self {
        Self {
            db,
            ttl_seconds: ttl_days * 86_400,
        }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn get_film(&self, slug: &str) -> AppResult<Option<film_cache::Model>> {
        let film = film_cache::Entity::find_by_id(slug.to_string())
            .one(&self.db)
            .await?;
        Ok(film.filter(|f| self.is_fresh(f.updated_at)))
    }

    pub async fn upsert_film(
        &self,
        slug: &str,
        tmdb_id: Option<i32>,
        title: &str,
        year: Option<i16>,
    ) -> AppResult<()> {
        let now = now_sec();
        let model = film_cache::ActiveModel {
            letterboxd_slug: Set(slug.to_string()),
            tmdb_id: Set(tmdb_id),
            title: Set(title.to_string()),
            year: Set(year.map(|y| y as i32)),
            updated_at: Set(now),
        };

        film_cache::Entity::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(film_cache::Column::LetterboxdSlug)
                    .update_columns([
                        film_cache::Column::TmdbId,
                        film_cache::Column::Title,
                        film_cache::Column::Year,
                        film_cache::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    pub async fn get_releases(
        &self,
        tmdb_id: i32,
        country: &str,
    ) -> AppResult<Option<(Vec<ReleaseDate>, Vec<ReleaseDate>)>> {
        let meta = release_cache_meta::Entity::find()
            .filter(release_cache_meta::Column::TmdbId.eq(tmdb_id))
            .filter(release_cache_meta::Column::Country.eq(country))
            .one(&self.db)
            .await?;

        let Some(meta) = meta else {
            return Ok(None);
        };
        if !self.is_fresh(meta.cached_at) {
            return Ok(None);
        }

        let rows = release_cache::Entity::find()
            .filter(release_cache::Column::TmdbId.eq(tmdb_id))
            .filter(release_cache::Column::Country.eq(country))
            .all(&self.db)
            .await?;

        let mut theatrical = Vec::new();
        let mut streaming = Vec::new();

        for row in rows {
            let Ok(date) = row.release_date.parse() else {
                continue;
            };
            let Some(kind) = ReleaseType::from_tmdb_code(row.release_type) else {
                continue;
            };
            let rd = ReleaseDate {
                date,
                release_type: kind,
                note: row.note,
            };
            match kind {
                ReleaseType::Theatrical => theatrical.push(rd),
                ReleaseType::Digital => streaming.push(rd),
            }
        }

        theatrical.sort_by_key(|r| r.date);
        streaming.sort_by_key(|r| r.date);

        Ok(Some((theatrical, streaming)))
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

    fn is_fresh(&self, cached_at: i64) -> bool {
        now_sec().saturating_sub(cached_at) <= self.ttl_seconds
    }
}

fn now_sec() -> i64 {
    jiff::Timestamp::now().as_second()
}
