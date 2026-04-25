{
  description = "Transparent email alias proxy — like SimpleLogin, built entirely on Cloudflare Workers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, git-hooks }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "clippy" "rustfmt" ];
          targets = [ "wasm32-unknown-unknown" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*assets/.*" path != null);
        };

        commonArgs = {
          inherit src;
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        pre-commit-check = git-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            check-json.enable = true;
            check-merge-conflicts.enable = true;
            check-toml.enable = true;
            check-yaml.enable = true;
            detect-private-keys.enable = true;
            end-of-file-fixer.enable = true;
            mixed-line-endings.enable = true;
            trim-trailing-whitespace.enable = true;
            nixpkgs-fmt.enable = true;
            rustfmt = {
              enable = true;
              packageOverrides.cargo = rustToolchain;
              packageOverrides.rustfmt = rustToolchain;
            };
          };
        };
      in
      {
        checks = {
          inherit pre-commit-check;

          tests = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });

          fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            wrangler
            worker-build
            wasm-pack
            binaryen
            nodejs_22
          ];
          shellHook = ''
            ${pre-commit-check.shellHook}
            echo "Cutout dev environment"
            echo "  wrangler dev        - Start local dev server"
            echo "  wrangler deploy     - Deploy to Cloudflare"
            echo "  nix flake check     - Run CI checks"
          '';
        };
      }
    );
}
