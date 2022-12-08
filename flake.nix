{
  description = "mpdsonic - expose your MPD collection via Subsonic API";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, ... } @ inputs: inputs.flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import inputs.nixpkgs { inherit system; overlays = [ (import inputs.rust-overlay) ]; };
      rust = pkgs.rust-bin.stable.latest;

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rust.default;

      commonArgs = {
        src = ./.;
        nativeBuildInputs = [ pkgs.pkgconfig pkgs.rustPlatform.bindgenHook ];
        buildInputs = [ pkgs.openssl pkgs.libnfs ];
      };

      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
        installCargoArtifactsMode = "use-zstd";
      });

      fmt = craneLib.cargoFmt (commonArgs // { });

      clippy = craneLib.cargoClippy (commonArgs // {
        inherit cargoArtifacts fmt;

        installCargoArtifactsMode = "use-zstd";
        cargoClippyExtraArgs = "-- --deny warnings";
      });

      test = craneLib.cargoNextest (commonArgs // {
        cargoArtifacts = clippy;

        installCargoArtifactsMode = "use-zstd";
      });

      mpdsonic = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = test;

        doCheck = false;
      });
    in
    {
      checks = {
        inherit mpdsonic;
      };

      packages.default = mpdsonic;

      apps.default = inputs.flake-utils.lib.mkApp
        {
          drv = pkgs.symlinkJoin {
            name = "mpdsonic";
            paths = [ mpdsonic ];

            buildInputs = [ pkgs.makeWrapper ];

            postBuild = ''
              wrapProgram $out/bin/mpdsonic \
                  --prefix PATH : "${pkgs.ffmpeg}/bin"
            '';
          };
        };

      devShells.default = pkgs.mkShell {
        inputsFrom = [ mpdsonic ];

        nativeBuildInputs = [
          (rust.default.override
            {
              extensions = [ "rust-src" ];
            })
          pkgs.ffmpeg
        ];
      };
    });
}
