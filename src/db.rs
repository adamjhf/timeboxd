use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;

use crate::error::AppResult;

pub async fn connect_and_migrate(database_url: &str) -> AppResult<DatabaseConnection> {
    let db = Database::connect(database_url).await?;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        "PRAGMA journal_mode=WAL".to_string(),
    ))
    .await?;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        "PRAGMA synchronous=NORMAL".to_string(),
    ))
    .await?;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        "PRAGMA cache_size=-64000".to_string(),
    ))
    .await?;

    migration::Migrator::up(&db, None).await?;

    Ok(db)
}
