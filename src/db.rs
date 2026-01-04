use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

use crate::error::AppResult;

const MIGRATION_001: &str = include_str!("../migrations/001_initial.sql");

pub async fn connect_and_migrate(database_url: &str) -> AppResult<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    run_sql(&db, MIGRATION_001).await?;
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
