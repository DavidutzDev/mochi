# A flake for a plugin's repository, for a build Mochi can't guess by
# itself, like extra native libraries: copy it next to mochi-plugin.toml.
# Without one, Mochi builds Rust, Node, Python and Go plugins with Nix from
# their lock files anyway. With it:
#
# - `mochi plugins install` builds it with `nix build` when Nix is
#   installed, instead of the manifest's `build` command.
# - home-manager can take the package as
#   `programs.mochi.plugins.<id>.package = inputs.<plugin>.packages.${pkgs.system}.default;`
#
# `mochi.lib.buildPlugin` follows the plugin's lock file, so there's no
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
