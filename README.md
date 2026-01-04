# timeboxd

Track upcoming theatrical and streaming releases for films in your Letterboxd watchlist.

## Features

- Fetches films from your Letterboxd watchlist
- Shows upcoming theatrical and streaming release dates for your selected country
- Caches film metadata and release dates to minimize API calls
- Filters to recent films (last 3 years by default)

## Requirements

- Rust 2024 edition
- SQLite database
- [TMDB API access token](https://www.themoviedb.org/settings/api) (optional, uses mock data if not provided)

## Configuration

Set environment variables in `.env` or via your shell:

```bash
# Server
HOST=0.0.0.0                  # Default: 0.0.0.0
PORT=3000                     # Default: 3000

# TMDB API
TMDB_ACCESS_TOKEN=your_token  # Required for real data
TMDB_BASE_URL=https://api.themoviedb.org/3  # Default
TMDB_RPS=4                    # Rate limit (requests/second), default: 4

# Database
DATABASE_URL=sqlite://timeboxd.db?mode=rwc  # Default

# Cache
CACHE_TTL_DAYS=7              # Cache expiry in days, default: 7

# Performance
MAX_CONCURRENT_REQUESTS=5     # Concurrent film processing, default: 5
LETTERBOXD_DELAY_MS=250       # Delay between Letterboxd page requests, default: 250ms

# Logging
RUST_LOG=info,timeboxd=debug  # Default: info,timeboxd=debug,sqlx=warn
```

## Running

### Development

```bash
# With devenv
devenv shell
cargo run

# Without devenv
cargo run
```

### Production

```bash
cargo build --release
./target/release/timeboxd
```

The server will start on `http://0.0.0.0:3000` by default.

## Usage

1. Navigate to `http://localhost:3000`
2. Enter your Letterboxd username
3. Select your country
4. View upcoming releases sorted by date

## How it Works

1. Scrapes your public Letterboxd watchlist
2. Resolves film titles to TMDB IDs using:
   - Letterboxd metadata
   - Film cache
   - TMDB search API
3. Fetches release dates from TMDB for your selected country
4. Caches results to reduce API load
5. Returns films with upcoming theatrical or streaming releases

## Tech Stack

- **Framework**: Axum (async web)
- **Database**: SQLite with SeaORM
- **Templates**: Hypertext (type-checked HTML)
- **APIs**: TMDB API, Letterboxd scraping
- **Rate Limiting**: Governor

## License

MIT
