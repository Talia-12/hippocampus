{
  inputs = {
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    nixpkgs,
    flake-parts,
    rust-overlay,
    ...
  }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];
      perSystem = { config, self', pkgs, lib, system, ... }:
        let
          diesel-cli = pkgs.diesel-cli.override {
            sqliteSupport = true;
            mysqlSupport = false;
          };


          runtimeDeps = [
          ];
          buildDeps = [
          ];
          devDeps = [
            diesel-cli
          ];

          mkDevShell = rustc: pkgs.mkShell {
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            buildInputs = runtimeDeps;
            nativeBuildInputs = buildDeps ++ devDeps ++ [ rustc ];
          };
        in {
          _module.args.pkgs = import nixpkgs {
            inherit system;

            overlays = [ (import rust-overlay) ];
          };

          # packages.default = self'.packages.example;
          devShells.default = self'.devShells.nightly;

          # packages.example = (rustPackage "foobar");
          # packages.example-base = (rustPackage "");

          devShells.nightly = (mkDevShell (pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default)));
          devShells.stable = (mkDevShell pkgs.rust-bin.stable.latest.default);
          # devshells.msrv = (mkDevShell pkgs.rust-bin.${msrv}.default);
        };
    };
}
