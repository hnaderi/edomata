{
  inputs = {
    devshell.url =
      "github:numtide/devshell?rev=3e0e60ab37cd0bf7ab59888f5c32499d851edb47";
    typelevel-nix = {
      url = "github:typelevel/typelevel-nix";
      inputs.devshell.follows = "devshell";
    };
    nixpkgs.follows = "typelevel-nix/nixpkgs";
    flake-utils.follows = "typelevel-nix/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, typelevel-nix, devshell }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ typelevel-nix.overlay ];
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
                libraries = with pkgs; [ s2n utf8proc openssl ];
              };
            };
          };
      in {
        devShell = mkShell pkgs.jdk8;

        devShells = {
          "temurin@8" = mkShell pkgs.temurin-bin-8;
          "temurin@11" = mkShell pkgs.temurin-bin-11;
          "temurin@17" = mkShell pkgs.temurin-bin-17;
        };

      });
}
