{
  description = "EdgeForge — local-first SSD-backed frontier-model runtime";
  inputs = { nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05"; flake-utils.url = "github:numtide/flake-utils"; rust-overlay.url = "github:oxalica/rust-overlay"; };
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let overlays = [ (import rust-overlay) ]; pkgs = import nixpkgs { inherit system overlays; };
          rust = pkgs.rust-bin.stable."1.82.0".default.override { extensions = ["rust-src" "rustfmt" "clippy"]; };
      in { devShells.default = pkgs.mkShell { buildInputs = [ rust pkgs.cmake pkgs.pkg-config pkgs.openssl pkgs.sqlite ]; RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library"; }; });
}
