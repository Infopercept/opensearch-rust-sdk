# rust nix templ

A simple Rust crate template GitHub Actions workflows and a Nix flake for development environment. 

The Github Actions run common cargo commands to validate package builds, tests, conforms to formatting using a [Nix Dev Environment](https://zero-to-nix.com/concepts/dev-env). 

Benefits of using this is that it ensures your package and development shell can build the project.

## Resources

1. Nix flakes and dev shells: https://fasterthanli.me/series/building-a-rust-service-with-nix/part-10
2. Nix Concepts: https://zero-to-nix.com/concepts
3. Official Nix Flakes Docs: https://nixos.wiki/wiki/Flakes
