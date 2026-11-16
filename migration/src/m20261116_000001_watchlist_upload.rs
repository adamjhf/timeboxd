use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Films were keyed by scraped Letterboxd slugs, which uploaded exports don't contain.
        manager.drop_table(Table::drop().table(FilmCache::Table).if_exists().to_owned()).await?;

        manager
            .create_table(
                Table::create()
                    .table(FilmCache::Table)
                    .col(string(FilmCache::LetterboxdUri).primary_key())
                    .col(integer_null(FilmCache::TmdbId))
                    .col(string(FilmCache::Title))
                    .col(integer_null(FilmCache::Year))
                    .col(string_null(FilmCache::PosterPath))
                    .col(big_integer(FilmCache::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Watchlist::Table)
                    .col(string(Watchlist::Token).primary_key())
                    .col(string_null(Watchlist::Username))
                    .col(text(Watchlist::Films))
                    .col(big_integer(Watchlist::UploadedAt))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Watchlist::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(FilmCache::Table).to_owned()).await?;

        manager
            .create_table(
                Table::create()
                    .table(FilmCache::Table)
                    .col(string(Alias::new("letterboxd_slug")).primary_key())
                    .col(integer_null(FilmCache::TmdbId))
                    .col(string(FilmCache::Title))
                    .col(integer_null(FilmCache::Year))
                    .col(string_null(FilmCache::PosterPath))
                    .col(big_integer(FilmCache::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum FilmCache {
    Table,
    LetterboxdUri,
    TmdbId,
    Title,
    Year,
    PosterPath,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Watchlist {
    Table,
    Token,
    Username,
    Films,
    UploadedAt,
}
