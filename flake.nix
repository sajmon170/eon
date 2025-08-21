{
  description = "Rust flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      {
        devShells.default = with pkgs; mkShell.override {
            stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
        } rec {
            nativeBuildInputs = [
              (rust-bin.stable.latest.default.override { extensions = [ "rust-src" "rust-analyzer" ]; })
              (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default))
              pkg-config
            ];
            buildInputs = [
            ];
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            LD_LIBRARY_PATH = lib.makeLibraryPath [];
          };
      }
    );
}
