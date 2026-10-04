{
  description = "Mochi, a desktop shell built around a central island";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Pinned through flake.lock. Update with `nix flake update quickshell`.
    quickshell = {
      url = "git+https://git.outfoxxed.me/quickshell/quickshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      quickshell,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system} system);
    in
    {
      packages = forAllSystems (
        pkgs: system: {
          default = self.packages.${system}.mochi;
          mochi = pkgs.callPackage ./packaging/nix/package.nix {
            quickshell = quickshell.packages.${system}.default;
          };
          # The documentation site, from docs/book. Its pages include the
          # modules' settings.toml files, so it needs the whole source.
          docs = pkgs.runCommand "mochi-docs" { nativeBuildInputs = [ pkgs.mdbook ]; } ''
            cp -r ${self} source
            chmod -R u+w source
            mdbook build source/docs/book --dest-dir $out
          '';
        }
      );

      # `nix flake check` builds the package, which runs the test suite, the
      # documentation site, and what the NixOS module installs.
      checks = forAllSystems (
        _: system: {
          package = self.packages.${system}.mochi;
          docs = self.packages.${system}.docs;
          # The user units a NixOS system with `programs.mochi` gets.
          nixos-module =
            (nixpkgs.lib.nixosSystem {
              modules = [
                self.nixosModules.default
                {
                  nixpkgs.hostPlatform = system;
                  boot.isContainer = true;
                  system.stateVersion = "25.11";
                  programs.mochi.enable = true;
                }
              ];
            }).config.environment.etc."systemd/user".source;
        }
      );

      devShells = forAllSystems (
        pkgs: system: {
          default = pkgs.mkShell {
            packages = [
              # Rust
              pkgs.cargo
              pkgs.rustc
              pkgs.clippy
              pkgs.rustfmt
              pkgs.rust-analyzer

              # Native dependencies: libpulse for the OSD module, dbus for zbus
              pkgs.pkg-config
              pkgs.libpulseaudio
              pkgs.dbus

              # The documentation site: `mdbook serve docs/book`
              pkgs.mdbook

              # UI
              quickshell.packages.${system}.default
              # qmlls and qmlformat, built against the same Qt as Quickshell
              pkgs.kdePackages.qtdeclarative

              # Visual checks: screenshots, recordings and real pointer events
              pkgs.grim
              pkgs.wlrctl
              pkgs.wf-recorder
            ];

            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        }
      );

      # `pkgs.mochi`, built against the pinned Quickshell.
      overlays.default = final: _: {
        mochi = final.callPackage ./packaging/nix/package.nix {
          quickshell = quickshell.packages.${final.stdenv.hostPlatform.system}.default;
        };
      };

      # `programs.mochi` for home-manager and NixOS, using this flake's
      # package unless `programs.mochi.package` says otherwise.
      homeModules.default =
        { lib, pkgs, ... }:
        {
          imports = [ ./packaging/nix/hm-module.nix ];
          programs.mochi.package = lib.mkDefault self.packages.${pkgs.stdenv.hostPlatform.system}.mochi;
        };

      nixosModules.default =
        { lib, pkgs, ... }:
        {
          imports = [ ./packaging/nix/nixos-module.nix ];
          programs.mochi.package = lib.mkDefault self.packages.${pkgs.stdenv.hostPlatform.system}.mochi;
        };

      formatter = forAllSystems (pkgs: _: pkgs.nixfmt);
    };
}
