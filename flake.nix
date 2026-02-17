{
  description = "A terminal-first GitHub pull request review client";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    difftastic-src = {
      url = "github:Wilfred/difftastic";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      difftastic-src,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        difftastic = pkgs.difftastic.overrideAttrs (old: {
          version = "unstable-${difftastic-src.shortRev or "dirty"}";
          src = difftastic-src;
          cargoDeps = rustPlatform.importCargoLock {
            lockFile = "${difftastic-src}/Cargo.lock";
          };
          doInstallCheck = false;
        });

        runtimeDeps = [
          difftastic
          pkgs.gh
        ];

        critic = rustPlatform.buildRustPackage {
          pname = "critic";
          version = "0.0.3";
          src = pkgs.lib.cleanSource ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            makeWrapper
          ];

          buildInputs =
            with pkgs;
            [ openssl ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          postInstall = ''
            wrapProgram $out/bin/critic \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
          '';

          meta = {
            description = "A terminal-first GitHub pull request review client";
            homepage = "https://github.com/clabby/critic";
            license = pkgs.lib.licenses.mit;
            mainProgram = "critic";
          };
        };
      in
      {
        packages.default = critic;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ critic ];
          packages =
            runtimeDeps
            ++ (with pkgs; [
              cargo-nextest
              just
              rust-analyzer
            ]);
        };
      }
    );
}
