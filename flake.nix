{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1"; # unstable Nixpkgs
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, ... }@inputs:

    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [
                inputs.self.overlays.default
              ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        rustToolchain =
          with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable;
            [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
              llvm-tools
            ]
          );

        zsh-bench = final.stdenvNoCC.mkDerivation {
          pname = "zsh-bench";
          version = "28b1b1b";

          src = final.fetchFromGitHub {
            owner = "romkatv";
            repo = "zsh-bench";
            rev = "28b1b1bc888159f0a2cf50f9d29381758341aba1";
            sha256 = "19j5pm498qm09jj3lziblzkysh6sc7dzykmqfj4kly7h6jjcdhbn";
          };

          nativeBuildInputs = [
            final.makeWrapper
          ];

          dontBuild = true;

          installPhase = ''
            runHook preInstall

            mkdir -p "$out/share/zsh-bench"
            cp -R . "$out/share/zsh-bench"

            makeWrapper "$out/share/zsh-bench/zsh-bench" "$out/bin/zsh-bench" \
              --prefix PATH : ${
                final.lib.makeBinPath [
                  final.coreutils
                  final.git
                  final.zsh
                ]
              }

            runHook postInstall
          '';
        };
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo-deny
              cargo-dist
              cargo-edit
              cargo-llvm-cov
              cargo-nextest
              cargo-watch
              git
              hyperfine
              just
              nixfmt
              openssl
              pkg-config
              rustToolchain
              rust-analyzer
              shellcheck
              taplo
              zsh
              zsh-bench
            ];

            env = {
              # Required by rust-analyzer
              RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };
          };
        }
      );

      formatter = forEachSupportedSystem ({ pkgs }: pkgs.nixfmt);
    };
}
