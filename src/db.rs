use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

use crate::error::AppResult;

const MIGRATION_001: &str = include_str!("../migrations/001_initial.sql");
const MIGRATION_002: &str = include_str!("../migrations/002_add_poster_path.sql");

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

    run_sql(&db, MIGRATION_001).await?;
    run_sql_ignore_duplicate_column(&db, MIGRATION_002).await?;
    Ok(db)
}

async fn run_sql(db: &DatabaseConnection, sql: &str) -> AppResult<()> {
    for stmt in sql.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        db.execute(Statement::from_string(db.get_database_backend(), stmt.to_string())).await?;
    }
    Ok(())
}

async fn run_sql_ignore_duplicate_column(db: &DatabaseConnection, sql: &str) -> AppResult<()> {
    for stmt in sql.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        if let Err(e) =
            db.execute(Statement::from_string(db.get_database_backend(), stmt.to_string())).await
        {
            let err_str = e.to_string();
            if !err_str.contains("duplicate column name") {
                return Err(e.into());
            }
        }
    }
    Ok(())
}
