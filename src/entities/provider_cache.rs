use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "provider_cache")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tmdb_id: i32,
    pub country: String,
    pub provider_id: i32,
    pub provider_name: String,
    pub logo_path: String,
    pub link: Option<String>,
    pub provider_type: i32,
    pub cached_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
