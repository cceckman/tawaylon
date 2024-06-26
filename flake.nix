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
      in rec {
        packages = {
          default = pkgs.writeShellScriptBin "build.sh" ''
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
              pkgs.stdenv.cc.cc.lib
              pkgs.alsa-lib
              pkgs.libxkbcommon
            ]}
            export PKG_CONFIG_PATH=${pkgs.lib.makeLibraryPath [
              pkgs.alsa-lib 
            ]}:${pkgs.alsa-lib.dev}/lib/pkgconfig:${pkgs.libxkbcommon.dev}/lib/pkgconfig
            PATH=$PATH:${pkgs.lib.makeBinPath [
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
            inputsFrom = [ packages.default ];
          };
        };
      });
}
