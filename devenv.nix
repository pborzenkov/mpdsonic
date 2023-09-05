{pkgs, ...}: {
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = [
    pkgs.pkg-config
    pkgs.openssl

    pkgs.cargo-nextest
  ];

  pre-commit = {
    hooks = {
      cargo-check.enable = true;
      clippy.enable = true;
      rustfmt.enable = true;
    };
    settings = {
      clippy.denyWarnings = true;
    };
  };
}
