{
  lib,
  rustPlatform,
  makeWrapper,
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
  # The capture module records through gpu-screen-recorder. Turn this off
  # when `recorder` in [module.capture] names another one.
  withGpuScreenRecorder ? true,
}:

let
  workspace = (lib.importTOML ../../Cargo.toml).workspace.package;
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

  nativeBuildInputs = [
    makeWrapper
    pkg-config
  ];
  # libpulse for the OSD module.
  buildInputs = [ libpulseaudio ];
  # The plugin installer's tests clone a repository of their own, and one
  # runs the Python example plugin.
  nativeCheckInputs = [
    git
    python3
  ];

  # The unit ships with /usr/bin paths; point them at this package. mochid
  # gets the Quickshell it was tested against, whatever is in the user's PATH,
  # and gpu-screen-recorder after it, so one installed system-wide wins.
  postInstall = ''
    install -Dm644 systemd/mochid.service $out/lib/systemd/user/mochid.service
    substituteInPlace $out/lib/systemd/user/mochid.service \
      --replace-fail /usr/bin/ $out/bin/
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
