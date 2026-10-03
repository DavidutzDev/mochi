{
  lib,
  rustPlatform,
  makeWrapper,
  pkg-config,
  libpulseaudio,
  quickshell,
}:

let
  workspace = (lib.importTOML ../Cargo.toml).workspace.package;
in
rustPlatform.buildRustPackage {
  pname = "mochi";
  inherit (workspace) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../modules
      ../systemd
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    makeWrapper
    pkg-config
  ];
  # libpulse for the OSD module.
  buildInputs = [ libpulseaudio ];

  # The unit ships with /usr/bin paths; point them at this package. mochid
  # gets the Quickshell it was tested against, whatever is in the user's PATH.
  postInstall = ''
    install -Dm644 systemd/mochid.service $out/lib/systemd/user/mochid.service
    substituteInPlace $out/lib/systemd/user/mochid.service \
      --replace-fail /usr/bin/ $out/bin/
    wrapProgram $out/bin/mochid --prefix PATH : ${lib.makeBinPath [ quickshell ]}
  '';

  meta = {
    description = "A desktop shell built around a central island";
    license = lib.licenses.mit;
    mainProgram = "mochid";
    platforms = lib.platforms.linux;
  };
}
