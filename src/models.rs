use jiff::civil::Date;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct WishlistFilm {
    pub letterboxd_slug: String,
    pub year: Option<i16>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ReleaseCategory {
    LocalUpcoming,
    LocalAlreadyAvailable,
    NoReleases,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ProviderType {
    Stream,
    Rent,
    Buy,
}

impl ProviderType {
    pub fn as_code(self) -> i32 {
        match self {
            ProviderType::Stream => 1,
            ProviderType::Rent => 2,
            ProviderType::Buy => 3,
        }
    }

    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(ProviderType::Stream),
            2 => Some(ProviderType::Rent),
            3 => Some(ProviderType::Buy),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WatchProvider {
    pub provider_id: i32,
    pub provider_name: String,
    pub logo_path: String,
    pub link: Option<String>,
    pub provider_type: ProviderType,
}

#[derive(Clone, Debug, Serialize)]
pub struct FilmWithReleases {
    pub title: String,
    pub year: Option<i16>,
    pub tmdb_id: i32,
    pub letterboxd_slug: String,
    pub poster_path: Option<String>,
    pub theatrical: Vec<ReleaseDate>,
    pub streaming: Vec<ReleaseDate>,
    pub category: ReleaseCategory,
    pub streaming_providers: Vec<WatchProvider>,
}

#[derive(Debug, Deserialize)]
pub struct TrackRequest {
    pub username: String,
    pub country: String,
}

#[derive(Clone, Debug)]
pub struct CountryReleases {
    pub country: String,
    pub theatrical: Vec<ReleaseDate>,
    pub streaming: Vec<ReleaseDate>,
}

#[derive(Clone, Debug)]
pub struct ReleaseDatesResult {
    pub requested_country: CountryReleases,
    pub all_countries: Vec<CountryReleases>,
}
