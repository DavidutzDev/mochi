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
    { nixpkgs, quickshell, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system} system);
    in
    {
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

              # Native dependencies for zbus and friends
              pkgs.pkg-config
              pkgs.dbus

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

      formatter = forAllSystems (pkgs: _: pkgs.nixfmt);
    };
}
