{
  description = "Rust flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      crane,
      advisory-db,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        inherit (pkgs) lib;
        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = [
          ];
        };
        
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        
        individualCrateArgs = commonArgs // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
          doCheck = false;
        };
        
        fileSetForCrate =
          crate:
          lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./eon-client
              ./libp2p-invert
              ./objects
              (craneLib.fileset.commonCargoSources crate)
            ];
          };
        eon-client = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "eon-client";
            cargoExtraArgs = "-p eon-client";
            src = fileSetForCrate ./eon-client;
          }
        );

        libp2p-invert = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "libp2p-invert";
            cargoExtraArgs = "-p libp2p-invert";
            src = fileSetForCrate ./libp2p-invert;
          }
        );

        objects = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "objects";
            cargoExtraArgs = "-p objects";
            src = fileSetForCrate ./objects;
          }
        );

      in
      {
        packages = {
          inherit eon-client libp2p-invert objects;
        };

        apps = {
          eon-client = flake-utils.lib.mkApp {
            drv = eon-client;
          };
        };

        devShells.default = with pkgs; mkShell.override {
            stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
        } rec {
            nativeBuildInputs = [
              (rust-bin.stable.latest.default.override { extensions = [ "rust-src" "rust-analyzer" ]; })
              (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default))
              pkg-config
              uv
            ];
            buildInputs = [
            ];
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            LD_LIBRARY_PATH = lib.makeLibraryPath [];
          };
      }
    );
}
