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
        requiredEnvs = ''
          PKG_CONFIG_PATH=$PKG_CONFIG_PATH:${pkgs.alsa-lib.dev}/lib/pkgconfig;
          PKG_CONFIG_PATH=$PKG_CONFIG_PATH:${pkgs.libxkbcommon.dev}/lib/pkgconfig;

          export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:${pkgs.alsa-lib}/lib;
          export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:${pkgs.libxkbcommon}/lib;
          export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:${pkgs.stdenv.cc.cc}/lib
        '';
        packages = {
          default = { };
        };
        devShells = {
          default = pkgs.mkShell { 
            shellHook = requiredEnvs;
            buildInputs = [pkgs.stdenv.cc.cc.lib];
            packages = [ 
              pkgs.unzip
              pkgs.rust-analyzer
              pkgs.cargo
            ]; 
          };
        };
      });
}
