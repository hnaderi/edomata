{
  inputs = {
    typelevel-nix.url = "github:typelevel/typelevel-nix";
    nixpkgs.follows = "typelevel-nix/nixpkgs";
    flake-utils.follows = "typelevel-nix/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, typelevel-nix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ typelevel-nix.overlays.default ];
        };

        mkShell = jdk:
          pkgs.devshell.mkShell {
            imports = [ typelevel-nix.typelevelShell ];
            name = "edomata-shell";
            typelevelShell = {
              jdk.package = jdk;
              nodejs.enable = true;
              native = {
                enable = true;
                libraries = with pkgs; [ s2n-tls utf8proc openssl ];
              };
            };
          };
      in {
        devShell = mkShell pkgs.jdk17;

        devShells = {
          "temurin@8" = mkShell pkgs.temurin-bin-8;
          "temurin@11" = mkShell pkgs.temurin-bin-11;
          "temurin@17" = mkShell pkgs.temurin-bin-17;
        };

      });
}
