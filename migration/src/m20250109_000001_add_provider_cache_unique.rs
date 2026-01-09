use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_provider_cache_unique")
                    .table(ProviderCache::Table)
                    .col(ProviderCache::TmdbId)
                    .col(ProviderCache::Country)
                    .col(ProviderCache::ProviderId)
                    .col(ProviderCache::ProviderType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_provider_cache_unique")
                    .table(ProviderCache::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ProviderCache {
    Table,
    TmdbId,
    Country,
    ProviderId,
    ProviderType,
}
