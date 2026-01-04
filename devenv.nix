{
  pkgs,
  ...
}:
{
  packages = [ pkgs.git ];

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
}
