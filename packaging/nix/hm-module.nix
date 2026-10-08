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
    ${lib.optionalString (cfg.widgets != null) ''
      cp ${widgetsFile} $out/widgets.toml
    ''}
    ${lib.getExe' package "mochid"} config check --config $out/config.toml
  '';

  # The declared widget layout, as Nix or as TOML text.
  widgetsFile =
    if builtins.isString cfg.widgets then
      pkgs.writeText "widgets.toml" cfg.widgets
    else
      toml.generate "widgets.toml" { widget = cfg.widgets; };

  # Plugins Nix builds go in the store, and plugins.toml points at them.
  buildPlugin = import ./build-plugin.nix { inherit pkgs; };
  pluginSource =
    plugin:
    if builtins.isString plugin then
      plugin
    else if plugin.source != null then
      plugin.source
    else if plugin.package != null then
      "path:${plugin.package}"
    else
      "path:${buildPlugin { inherit (plugin) src buildInputs runtimeInputs; }}";
  pluginList.plugins = lib.mapAttrs (_: plugin: { source = pluginSource plugin; }) cfg.plugins;

  pluginType = lib.types.either lib.types.str (
    lib.types.submodule {
      options = {
        source = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          example = "git:github.com/User/mochi-pomodoro:main";
          description = "A source for `mochi plugins install`, like the plain string form.";
        };
        src = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = null;
          example = lib.literalExpression "inputs.mochi-pomodoro";
          description = ''
            The plugin's source tree, usually a flake input with
            `flake = false`, or its release archive. Nix builds it during the
            switch, with no build tools on the machine and no
            `mochi plugins install`: Rust, Node, Python and Go (with
            vendor/) from their lock files, and scripts or release binaries
            as they are.
          '';
        };
        package = lib.mkOption {
          type = lib.types.nullOr lib.types.package;
          default = null;
          example = lib.literalExpression "inputs.mochi-pomodoro.packages.\${pkgs.system}.default";
          description = ''
            The plugin already built, like its own flake's package: a
            directory with {file}`mochi-plugin.toml`, its views and its
            backend.
          '';
        };
        buildInputs = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [ ];
          example = lib.literalExpression "[ pkgs.alsa-lib ]";
          description = "Native libraries the backend links, for `src`.";
        };
        runtimeInputs = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [ ];
          example = lib.literalExpression "[ pkgs.ffmpeg ]";
          description = ''
            Programs the backend runs, put on its PATH, for `src`. Most come
            from the `needs` in the plugin's manifest already.
          '';
        };
      };
    }
  );

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
        commented example on its first start. Without `modules`, every
        builtin module runs. `mochi config init --print`
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
      type = lib.types.attrsOf pluginType;
      default = { };
      example = lib.literalExpression ''
        {
          weather = "git-release:github.com/User/mochi-weather:v0.2.0";
          pomodoro.src = inputs.mochi-pomodoro;
          clock.package = inputs.mochi-clock.packages.''${pkgs.system}.default;
        }
      '';
      description = ''
        Plugins by id, written to {file}`$XDG_CONFIG_HOME/mochi/plugins.toml`.
        Enable them in `settings.modules` like builtin modules.

        A string is a source for `mochi plugins install`, which you run
        after switching; it fetches and builds the plugin and writes
        {file}`plugins.lock`. `src` has Nix build the plugin during the
        switch instead, with no build tools on the machine, and `package`
        takes one already built, like the plugin's own flake's package.
      '';
    };

    widgets = lib.mkOption {
      type = lib.types.nullOr (lib.types.either (lib.types.listOf toml.type) lib.types.lines);
      default = null;
      example = lib.literalExpression ''
        [
          {
            id = "w1";
            module = "widgets";
            widget = "clock";
            output = "DP-3";
            anchor = "top-left";
            x = 2;
            y = 3;
            width = 14;
            height = 7;
            settings.seconds = true;
          }
        ]
      '';
      description = ''
        The widget layout, as `mochi ipc widgets export` prints it, or the
        text of a {file}`widgets.toml`. It's written to
        {file}`$XDG_CONFIG_HOME/mochi/widgets.toml` as a file Mochi can
        change, so arranging the widgets keeps working; a rebuild writes it
        again only when this layout changed. To keep what you arranged,
        "Copy as Nix" in edit mode, or `mochi ipc widgets export`, and paste
        it here. Left `null`, home-manager doesn't touch the file.
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
    ]
    ++ lib.mapAttrsToList (id: plugin: {
      assertion =
        builtins.isString plugin
        ||
          lib.count (option: option != null) [
            plugin.source
            plugin.src
            plugin.package
          ] == 1;
      message = "programs.mochi.plugins.${id}: set exactly one of source, src and package";
    }) cfg.plugins;

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

    # A real file, not a link into the store, so the editor can rewrite it.
    # The stamp remembers which declared layout was written last, so a
    # rebuild that doesn't change it keeps what was arranged since.
    home.activation.mochiWidgets = lib.mkIf (cfg.widgets != null) (
      lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        dir="${config.xdg.configHome}/mochi"
        stamp="$dir/.widgets-declared"
        if [ "$(cat "$stamp" 2>/dev/null)" != "${widgetsFile}" ]; then
          run mkdir -p "$dir"
          run install -m 644 "${widgetsFile}" "$dir/widgets.toml"
          run sh -c 'echo "$1" > "$2"' sh "${widgetsFile}" "$stamp"
        fi
      ''
    );

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
