# Builds a plugin from its source with Nix: the plugin's directory as
# mochid reads it, with `mochi-plugin.toml`, the views and the backend at
# the `exec` its manifest names. Authors write no Nix and no hashes: the
# build follows the plugin's own lock file, chosen from its files, or
# `[backend] kind` in its manifest:
#
# - `rust`: Cargo.lock, git dependencies included.
# - `node`: package-lock.json; `npm run build` when package.json has one.
# - `python`: dependencies from pyproject.toml or requirements.txt, from
#   nixpkgs' Python packages.
# - `go`: go.mod with its dependencies in vendor/; without vendor/ Nix would
#   need a hash, so publish releases instead.
# - `files`: the plugin as it is, like a script or a release archive with
#   a built binary, which gets patched to run on NixOS.
#
# `[backend] needs` names the programs the backend runs; they go on its
# PATH. The flake has this as `lib.buildPlugin pkgs { src = ...; }`, and
# the home-manager module uses it for `programs.mochi.plugins.<id>.src`.
{ pkgs }:

{
  src,
  # Native libraries the backend links, like `[ pkgs.alsa-lib ]`.
  buildInputs ? [ ],
  nativeBuildInputs ? [ ],
  # Programs the backend runs, besides the manifest's `needs`.
  runtimeInputs ? [ ],
  ...
}@args:

let
  inherit (pkgs) lib;
  has = name: builtins.pathExists "${src}/${name}";
  manifest =
    if has "mochi-plugin.toml" then
      builtins.fromTOML (builtins.readFile "${src}/mochi-plugin.toml")
    else
      throw "buildPlugin: ${toString src} has no mochi-plugin.toml";
  id = manifest.plugin.id;
  version = manifest.plugin.version or "0";
  backend = manifest.backend or null;
  exec = backend.exec;

  kind =
    if backend == null then
      "files"
    else if backend ? kind then
      backend.kind
    else if has "Cargo.lock" then
      "rust"
    else if has "package-lock.json" then
      "node"
    else if has "go.mod" then
      "go"
    else if has "pyproject.toml" || has "requirements.txt" then
      "python"
    else if has exec then
      "files"
    else
      throw ''
        buildPlugin: can't tell how to build ${id}. It has no Cargo.lock,
        package-lock.json, go.mod, pyproject.toml or requirements.txt, and
        no ${exec} already built. Set `kind` in its manifest's [backend], or
        use a release archive as `src`.'';

  # Commands to the packages that have them, where the names differ. A
  # Python plugin's `python3` is the one with its dependencies.
  commands = with pkgs; {
    python3 = if kind == "python" then python else python3;
    python = if kind == "python" then python else python3;
    node = nodejs;
    npm = nodejs;
    java = jre;
    notify-send = libnotify;
    pactl = pulseaudio;
    paplay = pulseaudio;
    wl-copy = wl-clipboard;
    wl-paste = wl-clipboard;
    xdg-open = xdg-utils;
    magick = imagemagick;
    convert = imagemagick;
    dig = dnsutils;
  };
  needed =
    command:
    commands.${command} or (
      if builtins.hasAttr command pkgs && lib.isDerivation pkgs.${command} then
        pkgs.${command}
      else
        throw "buildPlugin: ${id} needs `${command}`, which no package by that name has; give its package in programs.mochi.plugins.${id}.runtimeInputs"
    );

  # Python packages from the plugin's dependency list, by their names in
  # nixpkgs: lower case, `_` and `.` read as `-`, versions and extras left.
  pythonNames =
    let
      fromPyproject =
        if has "pyproject.toml" then
          (builtins.fromTOML (builtins.readFile "${src}/pyproject.toml")).project.dependencies or [ ]
        else
          [ ];
      fromRequirements =
        if has "requirements.txt" then
          builtins.filter (line: line != "" && !(lib.hasPrefix "#" line) && !(lib.hasPrefix "-" line)) (
            map lib.trim (lib.splitString "\n" (builtins.readFile "${src}/requirements.txt"))
          )
        else
          [ ];
      name =
        requirement:
        lib.replaceStrings [ "_" "." ] [ "-" "-" ] (
          lib.toLower (builtins.head (builtins.match "([A-Za-z0-9._-]+).*" requirement))
        );
    in
    lib.unique (map name (fromPyproject ++ fromRequirements));
  python = pkgs.python3.withPackages (
    ps:
    map (
      name:
      ps.${name}
        or (throw "buildPlugin: ${id} depends on the Python package ${name}, which nixpkgs doesn't have")
    ) pythonNames
  );

  # The language's own runtime first, so scripts' `#!/usr/bin/env` lines
  # find it before anything else.
  runtime = lib.unique (
    lib.optional (kind == "python") python
    ++ lib.optional (kind == "node") pkgs.nodejs
    ++ map needed (backend.needs or [ ])
    ++ runtimeInputs
  );

  # Puts the backend at `exec`: the built program named like it, or the
  # only one built.
  placeExec = ''
    if [ ! -x "$out/${exec}" ]; then
      built="$out/bin/$(basename ${lib.escapeShellArg exec})"
      if [ ! -x "$built" ]; then
        set -- $out/bin/*
        if [ $# -ne 1 ]; then
          echo "buildPlugin: can't tell which program is ${exec}: $*" >&2
          exit 1
        fi
        built=$1
      fi
      install -Dm755 "$built" "$out/${exec}"
    fi
  '';

  # What every kind shares: the plugin's files next to the backend, and the
  # runtime programs on its PATH.
  common = {
    pname = "mochi-plugin-${id}";
    inherit version src;
    buildInputs = runtime ++ buildInputs;
    nativeBuildInputs = [ pkgs.makeWrapper ] ++ nativeBuildInputs;
    postFixup =
      lib.optionalString (backend != null && runtime != [ ]) ''
        wrapProgram "$out/${exec}" --prefix PATH : ${lib.makeBinPath runtime}
      ''
      + (args.postFixup or "");
    meta = {
      description = manifest.plugin.description or "A Mochi plugin";
    }
    // (args.meta or { });
  };
  extra = removeAttrs args [
    "src"
    "buildInputs"
    "nativeBuildInputs"
    "runtimeInputs"
    "postFixup"
    "meta"
  ];

  copySource = ''
    cp -r --no-preserve=mode ${src}/. $out/
    rm -rf $out/target $out/result
  '';

  builders = {
    rust = pkgs.rustPlatform.buildRustPackage (
      common
      // {
        cargoLock = {
          lockFile = "${src}/Cargo.lock";
          allowBuiltinFetchGit = true;
        };
        nativeBuildInputs = common.nativeBuildInputs ++ [ pkgs.pkg-config ];
        # The plugin's own tests may need a running Mochi.
        doCheck = false;
        postInstall = copySource + placeExec;
      }
      // extra
    );

    go = pkgs.buildGoModule (
      common
      // {
        vendorHash =
          if has "vendor" then
            null
          else
            throw "buildPlugin: ${id} is in Go without a vendor/ directory, so Nix would need a hash for its dependencies. Commit vendor/ (`go mod vendor`), or use its release archive as src.";
        doCheck = false;
        postInstall = copySource + placeExec;
      }
      // extra
    );

    node = pkgs.buildNpmPackage (
      common
      // {
        npmDeps = pkgs.importNpmLock { npmRoot = src; };
        npmConfigHook = pkgs.importNpmLock.npmConfigHook;
        dontNpmBuild =
          !(((builtins.fromJSON (builtins.readFile "${src}/package.json")).scripts or { }) ? build);
        installPhase = ''
          runHook preInstall
          mkdir -p $out
          cp -r . $out/
          chmod +x "$out/${exec}"
          runHook postInstall
        '';
      }
      // extra
    );

    files = pkgs.stdenv.mkDerivation (
      common
      // {
        # A built binary in the source, like a release's, gets the loader
        # and libraries NixOS has instead of /lib64's.
        nativeBuildInputs = common.nativeBuildInputs ++ [ pkgs.autoPatchelfHook ];
        buildInputs = common.buildInputs ++ [ pkgs.stdenv.cc.cc.lib ];
        dontConfigure = true;
        dontBuild = true;
        installPhase = ''
          runHook preInstall
          mkdir -p $out
          ${copySource}
          ${lib.optionalString (backend != null) ''chmod +x "$out/${exec}"''}
          runHook postInstall
        '';
      }
      // extra
    );

    python = builders.files;
  };
in
builders.${kind}
  or (throw "buildPlugin: ${id} has [backend] kind = \"${kind}\"; it takes rust, node, python, go or files")
