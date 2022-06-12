{
  description = "mpdsonic - expose your MPD collection via Subsonic API";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, ... } @ inputs: inputs.flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import inputs.nixpkgs { inherit system; overlays = [ (import inputs.rust-overlay) ]; };
    in
    {
      devShell = pkgs.mkShell {
        nativeBuildInputs = [
          (pkgs.rust-bin.stable.latest.default.override
            {
              extensions = [ "rust-src" ];
            })
        ];
      };
    });
}
