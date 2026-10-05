# The home-manager module: `programs.mochi`. The flake's
# `homeModules.default` imports it and sets `package` to the flake's build.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.mochi;
  toml = pkgs.formats.toml { };

  # The files home-manager links into ~/.config/mochi. mochid checks them as
  # it would at startup, so a typo fails the build instead of the shell.
  files = pkgs.runCommand "mochi-config" { } ''
    mkdir $out
    ${lib.optionalString (cfg.settings != { }) ''
      cp ${toml.generate "config.toml" cfg.settings} $out/config.toml
    ''}
    ${lib.optionalString (cfg.theme != { }) ''
      cp ${toml.generate "theme.toml" cfg.theme} $out/theme.toml
    ''}
    ${lib.optionalString (cfg.plugins != { }) ''
      cp ${toml.generate "plugins.toml" pluginList} $out/plugins.toml
    ''}
    ${lib.getExe' package "mochid"} config check --config $out/config.toml
  '';

  pluginList.plugins = lib.mapAttrs (_: source: { inherit source; }) cfg.plugins;

  written =
    lib.optional (cfg.settings != { }) "config.toml"
    ++ lib.optional (cfg.theme != { }) "theme.toml"
    ++ lib.optional (cfg.plugins != { }) "plugins.toml";

  # gpu-screen-recorder comes with the package unless the settings name
  # another recorder.
  ownRecorder = lib.hasAttrByPath [ "module" "capture" "recorder" ] cfg.settings;
  package =
    if ownRecorder then
      (cfg.package.override or (_: cfg.package)) { withGpuScreenRecorder = false; }
    else
      cfg.package;
in
{
  options.programs.mochi = {
    enable = lib.mkEnableOption "Mochi, a desktop shell built around a central island";

    package = lib.mkOption {
      type = lib.types.package;
      description = ''
        The Mochi package, with `mochid` and `mochi`. It brings
        gpu-screen-recorder for recordings, unless `settings` sets
        `module.capture.recorder`. Recording a region or a screen also needs
        gpu-screen-recorder's capture helper, which only the system can
        install: on NixOS, `programs.gpu-screen-recorder.enable = true`.
      '';
    };

    settings = lib.mkOption {
      inherit (toml) type;
      default = { };
      example = lib.literalExpression ''
        {
          modules = [ "idle" "osd" "workspaces" "media" "launcher" "hub" "power" ];
          module.idle.format = "HH:mm:ss";
          module.power.lock = [ "hyprlock" ];
          bubbles.media.area = "center-left";
        }
      '';
      description = ''
        The contents of {file}`$XDG_CONFIG_HOME/mochi/config.toml`. Left
        empty, home-manager doesn't manage the file and Mochi writes a
        commented example on its first start. `mochi config init --print`
        shows every option with its default.
      '';
    };

    theme = lib.mkOption {
      inherit (toml) type;
      default = { };
      example = lib.literalExpression ''
        {
          colors.accent = "#8aadf4";
          layout.mode = "notch";
          text.family = "Inter";
        }
      '';
      description = ''
        The contents of {file}`$XDG_CONFIG_HOME/mochi/theme.toml`. Left empty,
        home-manager doesn't manage the file and Mochi writes a commented
        example on its first start.
      '';
    };

    plugins = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = lib.literalExpression ''
        {
          pomodoro = "git:github.com/User/mochi-pomodoro:main";
          weather = "git-release:github.com/User/mochi-weather:v0.2.0";
        }
      '';
      description = ''
        Plugins by id and source, written to
        {file}`$XDG_CONFIG_HOME/mochi/plugins.toml`. Enable them in
        `settings.modules` like builtin modules. Nix doesn't fetch or build
        them: run `mochi plugins install` after switching, which also
        writes {file}`plugins.lock` next to it.
      '';
    };

    portalPicker.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Use Mochi's share module as the screen-share picker of
        xdg-desktop-portal-hyprland: writes {file}`$XDG_CONFIG_HOME/hypr/xdph.conf`
        with `screencopy:custom_picker_binary`. Without Mochi running, or
        without the share module, the portal's own picker opens instead.
        The portal reads the file when it starts.
      '';
    };

    systemd = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Start `mochid` with the graphical session, as a systemd user
          service. When `settings` or `theme` change, activation runs
          `mochi reload` instead of restarting the shell.
        '';
      };

      target = lib.mkOption {
        type = lib.types.str;
        default = config.wayland.systemd.target;
        defaultText = lib.literalExpression "config.wayland.systemd.target";
        example = "hyprland-session.target";
        description = "The systemd target that starts and stops `mochid`.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      (lib.hm.assertions.assertPlatform "programs.mochi" pkgs lib.platforms.linux)
    ];

    home.packages = [ package ];

    xdg.configFile =
      lib.genAttrs (map (name: "mochi/${name}") written) (name: {
        source = "${files}/${baseNameOf name}";
      })
      // lib.optionalAttrs cfg.portalPicker.enable {
        "hypr/xdph.conf".text = ''
          screencopy {
            custom_picker_binary = ${package}/bin/mochi-share-picker
          }
        '';
      };

    systemd.user.services.mochid = lib.mkIf cfg.systemd.enable {
      Unit = {
        Description = "Mochi desktop shell";
        PartOf = [ cfg.systemd.target ];
        After = [ cfg.systemd.target ];
        Requisite = [ cfg.systemd.target ];
        X-Reload-Triggers = map (name: "${files}/${name}") written;
      };

      Service = {
        ExecStart = lib.getExe' package "mochid";
        ExecReload = "${lib.getExe' package "mochi"} reload";
        Restart = "on-failure";
        RestartSec = 1;
        Slice = "app-graphical.slice";
      };

      Install.WantedBy = [ cfg.systemd.target ];
    };
  };
}
