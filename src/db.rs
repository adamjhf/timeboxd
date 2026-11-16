use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::error::AppResult;

pub async fn connect_and_migrate(database_url: &str) -> AppResult<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    db.execute_unprepared("PRAGMA journal_mode=WAL").await?;
    db.execute_unprepared("PRAGMA synchronous=NORMAL").await?;
    db.execute_unprepared("PRAGMA cache_size=-64000").await?;

    migration::Migrator::up(&db, None).await?;

    Ok(db)
}
