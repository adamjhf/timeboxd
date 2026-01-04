{
  pkgs,
  ...
}:
{
  packages = [
    pkgs.git
    pkgs.sqlite
    pkgs.sea-orm-cli
  ];

  languages.rust = {
    enable = true;
    channel = "nightly";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };

  git-hooks.hooks = {
    rustfmt.enable = true;
  };

  env = {
    TMDB_API_KEY = "dummy_tmdb_api_key_for_testing";
    TMDB_BASE_URL = "https://api.themoviedb.org/3";
    DATABASE_URL = "sqlite://timeboxd.db?mode=rwc";
    CACHE_TTL_DAYS = "7";
    TMDB_RPS = "4";
    MAX_CONCURRENT_REQUESTS = "5";
    LETTERBOXD_DELAY_MS = "250";
    RUST_LOG = "debug";
  };
}
