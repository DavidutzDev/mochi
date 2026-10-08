{
  lib,
  stdenv,
  rustPlatform,
  makeWrapper,
  installShellFiles,
  git,
  curl,
  gnutar,
  gzip,
  xz,
  python3,
  pkg-config,
  libpulseaudio,
  quickshell,
  gpu-screen-recorder,
  callPackage,
  # The capture module records through gpu-screen-recorder. Turn this off
  # when `recorder` in [module.capture] names another one.
  withGpuScreenRecorder ? true,
}:

let
  workspace = (lib.importTOML ../../Cargo.toml).workspace.package;
  fonts = callPackage ./fonts.nix { };
in
rustPlatform.buildRustPackage {
  pname = "mochi";
  inherit (workspace) version;

  src = lib.fileset.toSource {
    root = ../../.;
    fileset = lib.fileset.unions [
      ../../Cargo.toml
      ../../Cargo.lock
      ../../crates
      ../../examples
      ../../modules
      ../../systemd
    ];
  };

  cargoLock.lockFile = ../../Cargo.lock;

  # The demo module is for working on Mochi; the tests drive it.
  buildNoDefaultFeatures = true;
  checkNoDefaultFeatures = false;

  nativeBuildInputs = [
    makeWrapper
    installShellFiles
    pkg-config
  ];
  # libpulse for the OSD module.
  buildInputs = [ libpulseaudio ];
  # The plugin installer's tests clone a repository of their own, one runs
  # the Python example plugin, and one compiles every view with Quickshell.
  nativeCheckInputs = [
    git
    python3
    quickshell
  ];

  # The unit ships with /usr/bin paths; point them at this package. mochid
  # gets the Quickshell it was tested against, whatever is in the user's PATH,
  # and gpu-screen-recorder after it, so one installed system-wide wins.
  postInstall = ''
    install -Dm644 systemd/mochid.service $out/lib/systemd/user/mochid.service
    substituteInPlace $out/lib/systemd/user/mochid.service \
      --replace-fail /usr/bin/ $out/bin/
  ''
  + lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
    installShellCompletion --cmd mochi \
      --bash <($out/bin/mochi completions bash) \
      --fish <($out/bin/mochi completions fish) \
      --zsh <($out/bin/mochi completions zsh)
  ''
  + ''
    # xdg-desktop-portal-hyprland runs one program, without arguments of
    # our choosing, as its screen-share picker.
    makeWrapper $out/bin/mochi $out/bin/mochi-share-picker --add-flags share-pick
    # `mochi plugins` clones, downloads and unpacks with these, after
    # whatever the user has.
    wrapProgram $out/bin/mochi \
      --suffix PATH : ${
        lib.makeBinPath [
          git
          curl
          gnutar
          gzip
          xz
        ]
      }
    wrapProgram $out/bin/mochid \
      --prefix PATH : ${lib.makeBinPath [ quickshell ]} \
      --set-default MOCHI_FONTS ${fonts} \
      ${lib.optionalString withGpuScreenRecorder "--suffix PATH : ${
        lib.makeBinPath [ gpu-screen-recorder ]
      }"}
  '';

  meta = {
    description = "A desktop shell built around a central island";
    license = lib.licenses.mit;
    mainProgram = "mochid";
    platforms = lib.platforms.linux;
  };
}
