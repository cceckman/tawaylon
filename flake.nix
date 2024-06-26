{
  description = "";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in {
        packages = {
          default = pkgs.writeShellScriptBin "build.sh" ''
            LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
              pkgs.alsa-lib
              pkgs.libxkbcommon
              pkgs.libstdcxx5
            ]}
            PKG_CONFIG_PATH=${pkgs.lib.makeLibraryPath [
              pkgs.alsa-lib 
              "${pkgs.alsa-lib.dev}/pkgconfig"
            ]}
            ${pkgs.lib.makeBinPath [
              pkgs.libxkbcommon
              pkgs.stdenv.cc.cc.lib
              pkgs.rust-analyzer
              pkgs.python3
              pkgs.unzip
              pkgs.cargo
            ]}
            ./do tawaylon/run
          '';
        };
        devShells = {
          default = pkgs.mkShell { 
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ 
              pkgs.alsa-lib
              pkgs.libxkbcommon
              pkgs.libstdcxx5
            ];
            PKG_CONFIG_PATH = pkgs.lib.makeLibraryPath [
              pkgs.alsa-lib 
              "${pkgs.alsa-lib.dev}/pkgconfig"
            ];
            buildInputs = [
              pkgs.libxkbcommon
              pkgs.stdenv.cc.cc.lib
              pkgs.rust-analyzer
              pkgs.python3
              pkgs.unzip
              pkgs.cargo
            ];
          };
        };
      });
}
