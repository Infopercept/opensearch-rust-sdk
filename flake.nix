{
  description = "Crate for interacting with OpenSearch nodes";

  nixConfig.bash-prompt = "[nix(opensearch-sdk-rs)] ";
  inputs = { nixpkgs.url = "github:nixos/nixpkgs/23.11"; };

  outputs = { self, nixpkgs }:
    let
      pkgs = {
        x86_64-darwin = nixpkgs.legacyPackages.x86_64-darwin.pkgs;
        aarch64-darwin = nixpkgs.legacyPackages.aarch64-darwin.pkgs;
        x86_64-linux = nixpkgs.legacyPackages.x86_64-linux.pkgs;
        aarch64-linux = nixpkgs.legacyPackages.aarch64-linux.pkgs;
      };

      commonBuildInputs = pkgs:
        with pkgs; [
          jdk17
          nixpkgs-fmt
          protobuf
          libiconv
          just
          rustc
          rustfmt
          cargo
        ];

      darwinBuildInputs = pkgs: commonBuildInputs pkgs ++ [ pkgs.darwin.apple_sdk.frameworks.Security ];

      devShells = {
        x86_64-darwin.default = pkgs.x86_64-darwin.mkShell {
          buildInputs = darwinBuildInputs pkgs.x86_64-darwin;
        };

        aarch64-darwin.default = pkgs.aarch64-darwin.mkShell {
          buildInputs = darwinBuildInputs pkgs.aarch64-darwin;
        };

        x86_64-linux.default = pkgs.x86_64-linux.mkShell {
          buildInputs = commonBuildInputs pkgs.x86_64-linux;
        };

        aarch64-linux.default = pkgs.aarch64-linux.mkShell {
          buildInputs = commonBuildInputs pkgs.aarch64-linux;
        };
      };
    in
    {
      formatter.x86_64-darwin = pkgs.x86_64-darwin.nixpkgs-fmt;
      formatter.aarch64-darwin = pkgs.aarch64-darwin.nixpkgs-fmt;
      formatter.x86_64-linux = pkgs.x86_64-linux.nixpkgs-fmt;
      formatter.aarch64-linux = pkgs.aarch64-linux.nixpkgs-fmt;

      inherit devShells;
    };
}
