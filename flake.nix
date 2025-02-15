{
  description = "starter for packages wit nix and rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  # add "self" if useful here
  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
      in with pkgs; {
        devShells.default = mkShell {
          buildInputs = [
            rust-analyzer
            cargo
            cargo-edit

            # Other potentially useful packages
            # jq
            # openssl
            # pkg-config

            (rust-bin.selectLatestNightlyWith (toolchain:
              toolchain.default.override { extensions = [ "rust-src" ]; }))
          ];
        };
      });
}
