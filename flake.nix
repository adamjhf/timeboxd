{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    extra-container.url = "github:erikarvstedt/extra-container";
    extra-container.inputs.nixpkgs.follows = "nixpkgs";
    extra-container.inputs.flake-utils.follows = "flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      fenix,
      flake-utils,
      extra-container,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        profile = fenix.packages.${system}.complete;

        devRustToolchain = fenix.packages.${system}.combine [
          profile.cargo
          profile.rustc
          profile.clippy
          profile.rustfmt
          profile.rust-analyzer
          profile.rust-src
        ];
        buildRustToolchain = fenix.packages.${system}.combine [
          profile.cargo
          profile.rustc
        ];

        craneLib = (crane.mkLib pkgs).overrideToolchain buildRustToolchain;

        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
          doCheck = false;
          cargoCheckCommand = "${pkgs.coreutils}/bin/true";

          buildInputs = with pkgs; lib.optionals stdenv.isDarwin [ libiconv ];

          nativeBuildInputs =
            with pkgs;
            [
              cmake
              git
            ]
            ++ lib.optionals stdenv.isLinux [
              clang
              mold
            ];

          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER =
            if !pkgs.stdenv.isDarwin then "${pkgs.clang}/bin/clang" else null;
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS =
            if !pkgs.stdenv.isDarwin then "-C link-arg=-fuse-ld=${pkgs.mold}/bin/mold" else null;
        };

        timeboxd = craneLib.buildPackage (
          commonArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          }
        );
      in
      {
        checks = {
          inherit timeboxd;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = timeboxd;
        };

        devShells.default =
          let
            port = 3000;
          in
          ((crane.mkLib pkgs).overrideToolchain devRustToolchain).devShell {
            checks = self.checks.${system};

            packages = with pkgs; [
              bacon
              sea-orm-cli
              sqlite
            ];

            PORT = port;
            TMDB_BASE_URL = "https://api.themoviedb.org/3";
            DATABASE_URL = "sqlite://timeboxd.db?mode=rwc";
            CACHE_TTL_DAYS = "7";
            RELEASE_CACHE_HOURS = "24";
            TMDB_RPS = "10";
            MAX_CONCURRENT_REQUESTS = "5";
            LETTERBOXD_DELAY_MS = "100";
            RUST_LOG = "info,timeboxd=debug,tower_http=debug,sqlx=warn";
          };

        packages = {
          default = timeboxd;
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          container = extra-container.lib.buildContainers {
            inherit system nixpkgs;

            config.containers.timeboxd = {
              privateNetwork = false;
              autoStart = true;
              bindMounts = {
                "/run/secrets/timeboxdEnv" = {
                  hostPath = "/run/secrets/timeboxdEnv";
                  isReadOnly = true;
                };
                "/var/lib/timeboxd" = {
                  hostPath = "/var/lib/timeboxd";
                  isReadOnly = false;
                };
              };
              config.systemd.services."timeboxd" = {
                after = [ "network.target" ];
                wantedBy = [ "multi-user.target" ];
                serviceConfig = {
                  Type = "simple";
                  ExecStart = "${timeboxd}/bin/timeboxd";
                  Restart = "always";
                  EnvironmentFile = "/run/secrets/timeboxdEnv";
                  WorkingDirectory = "/var/lib/timeboxd";
                };
              };
            };
          };
        };
      }
    );
}
