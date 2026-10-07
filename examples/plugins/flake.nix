# A flake for a Rust plugin's repository: copy it next to
# mochi-plugin.toml. With it, people install the plugin without cargo:
#
# - `mochi plugins install` builds it with `nix build` when Nix is
#   installed, instead of the manifest's `build` command.
# - home-manager can take the package as
#   `programs.mochi.plugins.<id>.package = inputs.<plugin>.packages.${pkgs.system}.default;`
#
# `mochi.lib.buildPlugin` reads the crates from Cargo.lock, so there's no
# hash to update, and puts the backend at the manifest's `exec`.
{
  description = "A Mochi plugin";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    mochi = {
      url = "github:DavidutzDev/mochi";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, mochi, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        default = mochi.lib.buildPlugin pkgs {
          src = ./.;
          # Native libraries the backend links, if any.
          # buildInputs = [ pkgs.alsa-lib ];
        };
      });
    };
}
