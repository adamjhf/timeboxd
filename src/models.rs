use jiff::civil::Date;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct WishlistFilm {
    pub letterboxd_slug: String,
    pub title: String,
    pub year: Option<i16>,
    pub tmdb_id: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ReleaseType {
    Theatrical,
    Digital,
}

impl ReleaseType {
    pub fn as_tmdb_code(self) -> i32 {
        match self {
            ReleaseType::Theatrical => 3,
            ReleaseType::Digital => 4,
        }
    }

    pub fn from_tmdb_code(code: i32) -> Option<Self> {
        match code {
            3 => Some(ReleaseType::Theatrical),
            4 => Some(ReleaseType::Digital),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ReleaseDate {
    pub date: Date,
    pub release_type: ReleaseType,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FilmWithReleases {
    pub title: String,
    pub year: Option<i16>,
    pub tmdb_id: i32,
    pub theatrical: Vec<ReleaseDate>,
    pub streaming: Vec<ReleaseDate>,
}

impl FilmWithReleases {
    pub fn is_empty(&self) -> bool {
        self.theatrical.is_empty() && self.streaming.is_empty()
    }
}

#[derive(Debug, Deserialize)]
pub struct TrackRequest {
    pub username: String,
    pub country: String,
}
