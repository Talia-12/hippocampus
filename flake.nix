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
  } @ inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];
      
      flake = {
        # Define homeManagerModules at the flake level
        homeManagerModules.default = import ./module.nix inputs;
      };
      
      perSystem = { config, self', pkgs, lib, system, ... }:
        let
          diesel-cli = pkgs.diesel-cli.override {
            sqliteSupport = true;
            mysqlSupport = false;
          };

          runtimeDeps = [
            pkgs.openssl
            pkgs.pkg-config
          ];
          buildDeps = [
            pkgs.openssl
            pkgs.pkg-config
          ];
          devDeps = [
            diesel-cli
            pkgs.rustc.llvmPackages.llvm
          ];

          mkDevShell = rustc: pkgs.mkShell {
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            buildInputs = runtimeDeps;
            nativeBuildInputs = buildDeps ++ devDeps ++ [ rustc ];
          };
          
          mkRustPkg = rustc: pkgs.rustPlatform.buildRustPackage.override {
            rustc = rustc;
          } {
            pname = "hippocampus";
            version = "0.1.0";
            src = ./.;
            
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            
            nativeBuildInputs = buildDeps;
            buildInputs = runtimeDeps;

            checkFlags = [
              "--skip" "prop_test"
              "--skip" "proptests"
            ];
          };
        in {
          _module.args.pkgs = import nixpkgs {
            inherit system;

            overlays = [ (import rust-overlay) ];
          };

          packages.default = self'.packages.hippocampus;
          packages.hippocampus = mkRustPkg (pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default));
          
          devShells.default = self'.devShells.nightly;
          devShells.nightly = (mkDevShell (pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default)));
        };
    };
}
