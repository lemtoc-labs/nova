{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1"; # unstable Nixpkgs
    vhs-nixpkgs.url = "https://flakehub.com/f/DeterminateSystems/nixpkgs-weekly/0.1";
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
            inherit system;
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

      packages = forEachSupportedSystem (
        { pkgs, ... }:
        let
          nova = pkgs.rustPlatform.buildRustPackage {
            pname = "nova";
            version = "0.3.1";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeCheckInputs = [
              pkgs.git
            ];

            meta = {
              description = "A fast, customizable zsh prompt renderer.";
              homepage = "https://github.com/lemtoc-labs/nova";
              license = pkgs.lib.licenses.mit;
              mainProgram = "nova";
            };
          };
        in
        {
          inherit nova;
          default = nova;
        }
      );

      apps = forEachSupportedSystem (
        { system, ... }:
        {
          nova = {
            type = "app";
            program = "${self.packages.${system}.nova}/bin/nova";
          };
          default = self.apps.${system}.nova;
        }
      );

      devShells = forEachSupportedSystem (
        { pkgs, system, ... }:
        let
          vhsPkgs = import inputs.vhs-nixpkgs {
            inherit system;
          };
          fixedVhs = pkgs.writeShellScriptBin "vhs" ''
            export PATH="${
              pkgs.lib.makeBinPath [
                vhsPkgs.ttyd
                vhsPkgs.ffmpeg
              ]
            }:$PATH"
            exec ${vhsPkgs.vhs}/bin/.vhs-wrapped "$@"
          '';
        in
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
              fixedVhs
              zsh
              zsh-bench
            ];

            env = {
              # Required by rust-analyzer
              RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };

            shellHook = ''
              export PATH="${pkgs.rustToolchain}/bin:$PATH"
            '';
          };
        }
      );

      formatter = forEachSupportedSystem ({ pkgs, ... }: pkgs.nixfmt);
    };
}
