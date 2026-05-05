{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    nixpkgs-unstable.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    floresta-flake.url = "github:getfloresta/floresta-nix/stable_building";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      nixpkgs-unstable,
      floresta-flake,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs { inherit system overlays; };

        # We use src a lot across this flake
        src = ./.;

        inherit (floresta-flake.lib.${system}) florestaBuild;
      in
      with pkgs;
      {
        packages = {
          florestad = florestaBuild.build {
            inherit src;
            packageName = "florestad";
          };
          floresta-cli = florestaBuild.build {
            inherit src;
            packageName = "floresta-cli";
          };
          libfloresta = florestaBuild.build {
            inherit src;
            packageName = "libfloresta";
          };
          floresta-debug = florestaBuild.build {
            inherit src;
            packageName = "floresta-debug";
          };
          default = florestaBuild.build {
            inherit src;
            packageName = "all";
          };
        };
        devShells.default =
          let
            # This is the dev tools used while developing in Floresta.
            packages = with pkgs; [
              just
              rustup
              git
              boost
              cmake
              typos
              python312
              uv
              gcc
              go
              cargo-hack
            ];
          in
          mkShell {
            inherit packages;
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            CMAKE_PREFIX_PATH = "${pkgs.boost.dev}";
          };
      }
    );
}
