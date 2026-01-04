CREATE TABLE IF NOT EXISTS film_cache (
  letterboxd_slug TEXT PRIMARY KEY,
  tmdb_id INTEGER,
  title TEXT NOT NULL,
  year INTEGER,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS release_cache (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tmdb_id INTEGER NOT NULL,
  country TEXT NOT NULL,
  release_date TEXT NOT NULL,
  release_type INTEGER NOT NULL,
  note TEXT,
  cached_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_release_cache_unique
  ON release_cache (tmdb_id, country, release_date, release_type);

CREATE INDEX IF NOT EXISTS idx_film_cache_updated_at
  ON film_cache (updated_at);

CREATE INDEX IF NOT EXISTS idx_release_cache_tmdb_country
  ON release_cache (tmdb_id, country);

CREATE TABLE IF NOT EXISTS release_cache_meta (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tmdb_id INTEGER NOT NULL,
  country TEXT NOT NULL,
  cached_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_release_cache_meta_unique
  ON release_cache_meta (tmdb_id, country);
