# Agent Guidelines for timeboxd

This document provides essential information for agentic coding assistants working on the timeboxd Rust project.

## Project Overview

Timeboxd is a Rust web application that tracks upcoming theatrical and streaming releases for films in a user's Letterboxd watchlist. It fetches data from Letterboxd (via scraping) and TMDB API, caches results, and displays them in categorized sections with intelligent country fallbacks.

Built with:
- **Framework**: Axum (async web framework)
- **Database**: SQLite with SeaORM
- **External APIs**: TMDB API, Letterboxd scraping
- **Templates**: Hypertext (type-checked HTML with maud macro)
- **Error Handling**: anyhow with custom AppError wrapper

## Development Environment

The project uses devenv with Nix for development environment management. Key components:
- Rust 2024 edition
- SQLite database
- sea-orm-cli for database operations
- Bacon for auto-reload development
- Pre-commit hooks with rustfmt

### Environment Setup

```bash
# Activate devenv environment
devenv shell
```

## Build Commands

### Standard Build
```bash
cargo build
```

### Release Build
```bash
cargo build --release
```

### Check Compilation
```bash
cargo check
```

### Run Application
```bash
cargo run
```

**IMPORTANT**: When running the application in an agent context, DO NOT use `cargo run` directly as it will block indefinitely. Instead:
- Use background execution: `cargo run &` or `devenv shell -- cargo run &`
- Always clean up background processes before starting new ones: `pkill -9 -f timeboxd`
- If you get "Address already in use" error, use a different port: `PORT=3001 cargo run &`
- For testing, build only with `cargo build` and manually start the server outside the agent
- Never use tools that run the server in-process without background execution

## Testing Commands

### Run All Tests
```bash
cargo test
```

### Run Tests with Output
```bash
cargo test -- --nocapture
```

### Run Specific Test
```bash
cargo test test_name
```

### Run Tests in Specific File
```bash
cargo test --test integration_test_file
```

### Test with Coverage (requires cargo-tarpaulin)
```bash
cargo tarpaulin --ignore-tests
```

**Note**: Currently no tests exist in the codebase. When adding tests, follow Rust testing conventions.

## Linting and Formatting

### Format Code
```bash
cargo fmt
```

### Check Formatting
```bash
cargo fmt --check
```

### Clippy Lints
```bash
cargo clippy
```

### Fix Clippy Suggestions
```bash
cargo clippy --fix
```

## Code Style Guidelines

### General Principles
- Always use modern, idiomatic Rust patterns
- Leverage Rust's type system for safety and expressiveness
- Prefer functional patterns over imperative where appropriate
- Use idiomatic iterators and collection methods (map, filter, fold, etc.)
- Take advantage of Rust's ownership model for zero-cost abstractions
- **Edition**: Rust 2024
- **Formatting**: Enforced via rustfmt (automatic in pre-commit hooks)
- **Linting**: Use clippy for additional code quality checks
- **Error Handling**: Use `anyhow::Result<T>` for internal operations, `AppResult<T>` for HTTP handlers
- **Logging**: Use `tracing` crate for structured logging
- **Async**: Prefer async/await over futures/streams when possible

### Imports and Modules
- **Import grouping**: `imports_granularity = "Crate"` with `group_imports = "StdExternalCrate"`
- **Import order**: std library imports first, then external crates
- **Import layout**: Mixed layout (single line and multi-line as appropriate)
- **Example**:
  ```rust
  use std::collections::{HashMap, HashSet};

  use anyhow::anyhow;
  use axum::{Extension, Router};
  use hypertext::{maud, Renderable};
  use sea_orm::{Database, DatabaseConnection};
  use serde::{Deserialize, Serialize};

  use crate::{cache::CacheManager, config::Config, error::AppResult};
  ```

### Formatting Rules (from .rustfmt.toml)
- **Edition**: Rust 2024
- **Function params**: Tall layout (one parameter per line for multi-arg functions)
- **Trailing commas**: Vertical (always include trailing commas in structs/enums)
- **Match blocks**: Trailing comma required
- **Comments**: Wrapped at 100 characters, normalized, formatted in doc comments
- **Strings**: Formatted automatically
- **Newlines**: Unix style
- **Hex literals**: Lowercase

### Naming Conventions

- **Functions and variables**: `snake_case`
- **Types, structs, enums**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Route handlers**: `snake_case` describing the action (e.g., `get_user`, `add_entry`)
- **Modules**: snake_case (`mod film_cache;`)
- **Traits**: PascalCase ending with trait name (`Debug`, `Clone`)

### Type Definitions
- Use type aliases for common types where helpful
- Define request structs with `#[derive(Deserialize)]` for API inputs
- Define response structs with `#[derive(Serialize)]` for API outputs
- Use `Option<T>` for nullable fields
- Prefer explicit type signatures for public APIs

### Struct and Enum Definitions

```rust
// Prefer derive attributes in this order
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilmData {
    pub title: String,
    pub year: Option<i32>,
    pub tmdb_id: Option<i32>,
}

// Use meaningful field names
#[derive(Debug, Deserialize)]
pub struct ProcessQuery {
    username: String,
    country: String,
}
```

### Error Handling
- Use `AppResult<T>` which wraps `anyhow::Result<T>` for HTTP handlers
- Error type: `AppError` wrapping `anyhow::Error`
- Convert errors with `?` operator (auto-converts via From impl)
- Use `anyhow::anyhow!()` or `anyhow::Context` for error details

### Error Handling Patterns

```rust
// For HTTP handlers, use AppResult<T>
pub async fn process_handler(params: Params) -> AppResult<Html<String>> {
    // Validation with early returns
    if params.username.is_empty() {
        return Err(anyhow::anyhow!("username is required").into());
    }

    // Use ? for propagating errors in async contexts
    let data = fetch_data(&client).await?;

    Ok(Html(render_template(data)))
}

// For internal functions, use anyhow::Result<T>
async fn fetch_data(client: &reqwest::Client) -> anyhow::Result<FilmData> {
    // Use anyhow::bail! for early returns with context
    if !is_valid_request() {
        anyhow::bail!("invalid request parameters");
    }

    // Chain operations with context
    let response = client.get(url).send().await
        .context("failed to fetch from API")?;
    let data = response.json().await
        .context("failed to parse JSON response")?;

    Ok(data)
}
```

### Asynchronous Code
- Use `async fn` for async functions
- Use `tokio::join!` for parallel independent operations
- Use `tokio::try_join!` for parallel operations that can fail
- Use `match` and `?` for proper error handling

### Async Patterns

```rust
// Use tokio::main for main function
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Spawn tasks for concurrent operations
    let handle = tokio::spawn(async move {
        process_items(items).await
    });

    // Use Arc for shared state in async contexts
    let shared_state = Arc::new(AppState { /* ... */ });

    // Prefer async closures when possible
    let results: Vec<_> = stream::iter(items)
        .map(|item| async move { process_item(item).await })
        .buffer_unordered(5)
        .collect()
        .await;
}
```

### Database Operations (SeaORM)

```rust
// Entity definitions follow SeaORM conventions
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "film_cache")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub letterboxd_slug: String,
    pub tmdb_id: Option<i32>,
    pub title: String,
    pub year: Option<i32>,
    pub poster_path: Option<String>,
    pub updated_at: i64,
}

// Use ActiveModel for inserts/updates
let film = film_cache::ActiveModel {
    letterboxd_slug: Set(slug.to_string()),
    tmdb_id: Set(tmdb_id),
    title: Set(title.to_string()),
    year: Set(year.map(|y| y as i32)),
    poster_path: Set(poster_path),
    updated_at: Set(now),
    ..Default::default()
};
film.insert(&db).await?;
```

### HTTP Handler Patterns

```rust
use axum::{
    extract::{Form, Query, State},
    response::{Html, IntoResponse},
};

// Extractors in parameter order: State, Form/Query, then others
pub async fn track_handler(
    State(state): State<Arc<AppState>>,
    Form(req): Form<TrackRequest>,
) -> AppResult<Html<String>> {
    // Input validation
    let username = req.username.trim();
    if username.is_empty() {
        return Err(anyhow::anyhow!("username required").into());
    }

    // Business logic
    let result = process_request(&state, username).await?;

    Ok(Html(templates::render_result(result)))
}
```

### Configuration and State Management

```rust
// Use Arc for shared, immutable state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    pub cache: CacheManager,
    pub tmdb: Arc<TmdbClient>,
}

// Configuration from environment variables
pub struct Config {
    pub addr: SocketAddr,
    pub tmdb_access_token: String,
    pub tmdb_base_url: String,
    pub database_url: String,
    pub cache_ttl_days: i64,
    pub release_cache_hours: i64,
    pub tmdb_rps: u32,
    pub max_concurrent: usize,
    pub letterboxd_delay_ms: u64,
}
```

### Logging Patterns

```rust
// Use tracing macros with structured fields
tracing::info!(username = %username, "processing user request");
tracing::debug!(film_count = films.len(), "fetched films from cache");
tracing::error!(error = %err, "failed to process request");

// Use tracing subscriber in main
tracing_subscriber::fmt()
    .with_env_filter(
        std::env::var("RUST_LOG")
            .unwrap_or_else(|_| "info,timeboxd=debug".to_string()),
    )
    .init();
```

### Module Organization

- **main.rs**: Application entry point and setup
- **config.rs**: Configuration loading and validation
- **db.rs**: Database connection and migrations
- **entities/**: SeaORM entity definitions (generated)
  - **mod.rs**: Entity module exports
  - **film_cache.rs**: Film cache entity
  - **release_cache.rs**: Release cache entity
  - **release_cache_meta.rs**: Release cache metadata entity
- **models.rs**: Request/response models and data structures
- **routes.rs**: HTTP route handlers
- **processor.rs**: Film processing and categorization logic
- **scraper.rs**: Letterboxd watchlist scraping
- **tmdb.rs**: TMDB API client with rate limiting
- **cache.rs**: Caching layer for films and releases
- **templates.rs**: HTML template rendering with Hypertext
- **countries.rs**: Country code mappings
- **error.rs**: Error types and conversions

### General Guidelines
- Use `#![warn(clippy::all)]` at crate root (see main.rs)
- Prefer explicit type signatures for public APIs
- Log with `tracing` macros: `debug!`, `info!`, `error!`
- Keep functions focused and reasonably sized
- **Comments**: Use comments sparingly and only where code is non-intuitive or complex. Prefer self-documenting code over comments. Avoid redundant comments that restate what the code obviously does
- No documentation updates unless explicitly requested
- No emoji unless explicitly requested

### Dependencies and Versioning

- Use workspace dependencies in Cargo.toml for consistency
- Pin major versions for stability
- Use feature flags appropriately (e.g., SeaORM with SQLite)
- Keep dependencies minimal and up-to-date

### Security Considerations

- Validate all user inputs
- Use HTTPS for external API calls
- Implement rate limiting where appropriate
- Sanitize data before database operations
- Use secure defaults for configuration

### Performance Guidelines

- Use connection pooling for database operations
- Implement caching for expensive operations
- Use streaming for large data processing
- Consider memory usage with large datasets
- Profile performance-critical code paths

## File Structure Conventions

```
src/
├── main.rs              # Application entry point
├── config.rs            # Configuration management
├── db.rs               # Database setup and migrations
├── error.rs            # Error types and conversions
├── models.rs           # Request/response models
├── countries.rs        # Country code mappings
├── entities/           # SeaORM entities (generated)
│   ├── mod.rs
│   ├── film_cache.rs
│   ├── release_cache.rs
│   └── release_cache_meta.rs
├── routes.rs           # HTTP route handlers
├── processor.rs        # Film processing and categorization
├── scraper.rs          # Letterboxd watchlist scraping
├── tmdb.rs             # TMDB API client with rate limiting
├── cache.rs            # Caching layer for films and releases
└── templates.rs        # HTML template rendering with Hypertext

migrations/             # Database migrations
├── 001_initial.sql
└── 002_add_poster_path.sql

bacon.toml             # Development auto-reload configuration
README.md              # Project documentation
```

This document should be updated as the codebase evolves and new patterns emerge.
