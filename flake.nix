{
  description = "Nixpkgs stats.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    nil.url = "github:oxalica/nil";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = { self, nixpkgs, advisory-db, crane, nil, rust-overlay, ... }:
    let
      system = "x86_64-linux";
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        targets = [ "x86_64-unknown-linux-musl" ];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      src = pkgs.lib.cleanSourceWith {
        src = craneLib.path ./.;
        filter = path: type: (builtins.match ".*graphql$" path != null) || (craneLib.filterCargoSources path type);
      };

      commonArgs = {
        inherit src;

        strictDeps = true;
        buildInputs = [ ];

        CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
        CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      myCrate = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
      });
    in
    {
      devShells."${system}".default = pkgs.mkShell {
        buildInputs = with pkgs; [
          git
          git-lfs

          rustToolchain

          graphql-client

          nixpkgs-fmt
          nil.packages.${system}.default
        ];

        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
      };

      checks.${system} = {
        inherit myCrate;

        myCrateClippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
        });

        my-crate-fmt = craneLib.cargoFmt {
          inherit src;
        };

        my-crate-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };
      };

      packages.${system}.default = myCrate;
    };
}
