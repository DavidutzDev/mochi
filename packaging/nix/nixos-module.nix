# The NixOS module: `programs.mochi`. It installs Mochi for every user and
# starts it with each user's graphical session. Per-user settings belong in
# the home-manager module. The flake's `nixosModules.default` imports this
# and sets `package` to the flake's build.
{ config, lib, ... }:

let
  cfg = config.programs.mochi;
in
{
  options.programs.mochi = {
    enable = lib.mkEnableOption "Mochi, a desktop shell built around a central island";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The Mochi package, with `mochid` and `mochi`.";
    };

    recording.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Enable `programs.gpu-screen-recorder`, which the capture module
        records with: the program and the helper it needs to capture a
        region or a screen.
      '';
    };

    systemd.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Start `mochid` with every user's `graphical-session.target`. Turn
        this off to start it from your compositor or session manager
        instead.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    programs.gpu-screen-recorder.enable = lib.mkIf cfg.recording.enable (lib.mkDefault true);
    # The package's user unit, mochid.service.
    systemd.packages = [ cfg.package ];
    # Through the target rather than `systemd.user.services.mochid`: NixOS
    # would add a drop-in with its own PATH, and the launcher and the power
    # module need the user's.
    systemd.user.targets.graphical-session.wants = lib.mkIf cfg.systemd.enable [ "mochid.service" ];
  };
}
