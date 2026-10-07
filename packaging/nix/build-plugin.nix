# Builds a Rust plugin from its source with Nix: the plugin's directory as
# mochid reads it, with `mochi-plugin.toml`, the views and the backend at the
# `exec` its manifest names. No cargo needed on the machine, and no hash to
# write: the crates come from the plugin's Cargo.lock, git ones included.
#
# The flake has it as `lib.buildPlugin pkgs { src = ...; }`, and the
# home-manager module uses it for `programs.mochi.plugins.<id>.src`.
{
  lib,
  rustPlatform,
  pkg-config,
}:

{
  src,
  # Native libraries the backend links, like `[ pkgs.alsa-lib ]`.
  buildInputs ? [ ],
  nativeBuildInputs ? [ ],
  ...
}@args:

let
  manifestFile = "${src}/mochi-plugin.toml";
  manifest =
    if builtins.pathExists manifestFile then
      builtins.fromTOML (builtins.readFile manifestFile)
    else
      throw "buildPlugin: ${toString src} has no mochi-plugin.toml";
  id = manifest.plugin.id;
  exec =
    manifest.backend.exec
      or (throw "buildPlugin: ${id} has no backend to build; use its directory as a path: source");
  lockFile =
    if builtins.pathExists "${src}/Cargo.lock" then
      "${src}/Cargo.lock"
    else
      throw "buildPlugin: ${id} has no Cargo.lock; only Rust plugins build this way for now, so use the plugin's own flake package";
in
rustPlatform.buildRustPackage (
  removeAttrs args [
    "buildInputs"
    "nativeBuildInputs"
  ]
  // {
    pname = "mochi-plugin-${id}";
    version = manifest.plugin.version or "0";
    inherit src buildInputs;
    nativeBuildInputs = [ pkg-config ] ++ nativeBuildInputs;
    cargoLock = {
      inherit lockFile;
      allowBuiltinFetchGit = true;
    };
    # The plugin's own tests may need a running Mochi.
    doCheck = args.doCheck or false;

    # Everything the plugin ships next to the backend, then the backend at
    # its `exec`: the binary named like it, or the only one built.
    postInstall = ''
      cp -r --no-preserve=mode ${src}/. $out/
      rm -rf $out/target $out/result
      if [ ! -x "$out/${exec}" ]; then
        built="$out/bin/$(basename ${lib.escapeShellArg exec})"
        if [ ! -x "$built" ]; then
          set -- $out/bin/*
          if [ $# -ne 1 ]; then
            echo "buildPlugin: can't tell which binary is ${exec}: $*" >&2
            exit 1
          fi
          built=$1
        fi
        install -Dm755 "$built" "$out/${exec}"
      fi
      chmod +x "$out/${exec}"
    ''
    + (args.postInstall or "");

    meta = {
      description = manifest.plugin.description or "A Mochi plugin";
    }
    // (args.meta or { });
  }
)
