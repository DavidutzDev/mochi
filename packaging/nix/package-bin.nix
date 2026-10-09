# Mochi from its latest GitHub release, already built: nothing compiles.
# `release.json`, which the release workflow writes after publishing,
# names the archive for each system and its hash. The binaries are patched
# to find this system's libraries, and wrapped like the built package.
{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  makeWrapper,
  libpulseaudio,
  git,
  curl,
  gnutar,
  gzip,
  xz,
  quickshell,
  gpu-screen-recorder,
  withGpuScreenRecorder ? true,
}:

let
  release = lib.importJSON ./release.json;
  asset =
    release.assets.${stdenv.hostPlatform.system}
      or (throw "Mochi publishes no release for ${stdenv.hostPlatform.system}");
in
stdenv.mkDerivation {
  pname = "mochi-bin";
  inherit (release) version;

  src = fetchurl { inherit (asset) url sha256; };

  nativeBuildInputs = [
    autoPatchelfHook
    makeWrapper
  ];
  buildInputs = [
    libpulseaudio
    stdenv.cc.cc.lib
  ];

  dontBuild = true;

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -r bin share lib $out/
    substituteInPlace $out/lib/systemd/user/mochid.service \
      $out/share/applications/mochi-links.desktop \
      --replace-fail /usr/bin/ $out/bin/
    runHook postInstall
  '';

  # As the built package: the share picker xdg-desktop-portal-hyprland runs,
  # the tools `mochi plugins` uses, and Quickshell and the fonts for mochid.
  postFixup = ''
    makeWrapper $out/bin/mochi $out/bin/mochi-share-picker --add-flags share-pick
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
      --set-default MOCHI_FONTS $out/share/mochi/fonts \
      ${lib.optionalString withGpuScreenRecorder "--suffix PATH : ${
        lib.makeBinPath [ gpu-screen-recorder ]
      }"}
  '';

  meta = {
    description = "A desktop shell built around a central island, from its release";
    homepage = "https://github.com/DavidutzDev/mochi";
    license = lib.licenses.mit;
    mainProgram = "mochid";
    platforms = builtins.attrNames release.assets;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
}
