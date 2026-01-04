{
  config,
  pkgs,
  ...
}:
{
  packages = [
    pkgs.git
    pkgs.sqlite
    pkgs.sea-orm-cli
    pkgs.secretspec
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
    TMDB_ACCESS_TOKEN = config.secretspec.secrets.TMDB_ACCESS_TOKEN;
    TMDB_BASE_URL = "https://api.themoviedb.org/3";
    DATABASE_URL = "sqlite://timeboxd.db?mode=rwc";
    CACHE_TTL_DAYS = "7";
    TMDB_RPS = "4";
    MAX_CONCURRENT_REQUESTS = "5";
    LETTERBOXD_DELAY_MS = "250";
    RUST_LOG = "info,timeboxd=debug,sqlx=warn";
  };
}
