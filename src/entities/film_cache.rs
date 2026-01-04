use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "film_cache")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub letterboxd_slug: String,
    pub tmdb_id: Option<i32>,
    pub title: String,
    pub year: Option<i32>,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
