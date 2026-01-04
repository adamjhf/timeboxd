use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "release_cache")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tmdb_id: i32,
    pub country: String,
    pub release_date: String,
    pub release_type: i32,
    pub note: Option<String>,
    pub cached_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
