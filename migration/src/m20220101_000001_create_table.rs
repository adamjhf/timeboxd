use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FilmCache::Table)
                    .if_not_exists()
                    .col(string(FilmCache::LetterboxdSlug).primary_key())
                    .col(integer_null(FilmCache::TmdbId))
                    .col(string(FilmCache::Title))
                    .col(integer_null(FilmCache::Year))
                    .col(string_null(FilmCache::PosterPath))
                    .col(big_integer(FilmCache::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_film_cache_updated_at")
                    .table(FilmCache::Table)
                    .col(FilmCache::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ReleaseCache::Table)
                    .if_not_exists()
                    .col(pk_auto(ReleaseCache::Id))
                    .col(integer(ReleaseCache::TmdbId))
                    .col(string(ReleaseCache::Country))
                    .col(string(ReleaseCache::ReleaseDate))
                    .col(integer(ReleaseCache::ReleaseType))
                    .col(string_null(ReleaseCache::Note))
                    .col(big_integer(ReleaseCache::CachedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_release_cache_unique")
                    .table(ReleaseCache::Table)
                    .col(ReleaseCache::TmdbId)
                    .col(ReleaseCache::Country)
                    .col(ReleaseCache::ReleaseDate)
                    .col(ReleaseCache::ReleaseType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_release_cache_tmdb_country")
                    .table(ReleaseCache::Table)
                    .col(ReleaseCache::TmdbId)
                    .col(ReleaseCache::Country)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ReleaseCacheMeta::Table)
                    .if_not_exists()
                    .col(pk_auto(ReleaseCacheMeta::Id))
                    .col(integer(ReleaseCacheMeta::TmdbId))
                    .col(string(ReleaseCacheMeta::Country))
                    .col(big_integer(ReleaseCacheMeta::CachedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_release_cache_meta_unique")
                    .table(ReleaseCacheMeta::Table)
                    .col(ReleaseCacheMeta::TmdbId)
                    .col(ReleaseCacheMeta::Country)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(ReleaseCacheMeta::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(ReleaseCache::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(FilmCache::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum FilmCache {
    Table,
    LetterboxdSlug,
    TmdbId,
    Title,
    Year,
    PosterPath,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ReleaseCache {
    Table,
    Id,
    TmdbId,
    Country,
    ReleaseDate,
    ReleaseType,
    Note,
    CachedAt,
}

#[derive(DeriveIden)]
enum ReleaseCacheMeta {
    Table,
    Id,
    TmdbId,
    Country,
    CachedAt,
}
