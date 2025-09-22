{
  description = "Fire in the Term Nix flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        fittPkg = pkgs.rustPlatform.buildRustPackage {
          pname = "fire_in_the_term";
          version = "0.1.1";

          src = ./.;
          binaries = [ "fitt" ];

          cargoHash = "sha256-lR1sVoCof56k+ERDQlCyPLBBoLLctrmRe4sY64OH8EU=";
        };
      in
      {
        devShell = pkgs.mkShell {
          packages = [
            pkgs.rust-bin.stable.latest.default
          ];
        };

        packages.default = fittPkg;

        apps.default = {
          type = "app";
          program = "${fittPkg}/bin/fitt";
        };
      }
    );
}
