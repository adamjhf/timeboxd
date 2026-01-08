use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProviderCache::Table)
                    .if_not_exists()
                    .col(pk_auto(ProviderCache::Id))
                    .col(integer(ProviderCache::TmdbId))
                    .col(string(ProviderCache::Country))
                    .col(integer(ProviderCache::ProviderId))
                    .col(string(ProviderCache::ProviderName))
                    .col(string(ProviderCache::LogoPath))
                    .col(string_null(ProviderCache::Link))
                    .col(integer(ProviderCache::ProviderType))
                    .col(big_integer(ProviderCache::CachedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_provider_cache_tmdb_country")
                    .table(ProviderCache::Table)
                    .col(ProviderCache::TmdbId)
                    .col(ProviderCache::Country)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProviderCacheMeta::Table)
                    .if_not_exists()
                    .col(pk_auto(ProviderCacheMeta::Id))
                    .col(integer(ProviderCacheMeta::TmdbId))
                    .col(string(ProviderCacheMeta::Country))
                    .col(big_integer(ProviderCacheMeta::CachedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_provider_cache_meta_unique")
                    .table(ProviderCacheMeta::Table)
                    .col(ProviderCacheMeta::TmdbId)
                    .col(ProviderCacheMeta::Country)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(ProviderCacheMeta::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(ProviderCache::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ProviderCache {
    Table,
    Id,
    TmdbId,
    Country,
    ProviderId,
    ProviderName,
    LogoPath,
    Link,
    ProviderType,
    CachedAt,
}

#[derive(DeriveIden)]
enum ProviderCacheMeta {
    Table,
    Id,
    TmdbId,
    Country,
    CachedAt,
}
