# Installing

## Nix

The flake has the `mochi` package, with both binaries, the systemd unit and the Quickshell version `mochid` expects. It also has a home-manager module, a NixOS module and an overlay. Add it to your flake's inputs, pinned to a release:

```nix
mochi = {
  url = "github:DavidutzDev/mochi/v0.0.3";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

With `follows`, Mochi builds with your nixpkgs, which needs Rust 1.98 or newer. Quickshell stays pinned by Mochi's own lock file either way.

### home-manager

Import `inputs.mochi.homeModules.default`, then:

```nix
programs.mochi = {
  enable = true;

  # config.toml, as Nix. Leave it out and Mochi writes its commented
  # example on the first start instead.
  settings = {
    modules = [ "idle" "osd" "workspaces" "media" "notifications" "launcher" "hub" "power" ];
    module.idle.format = "HH:mm:ss";
    module.power.lock = [ "hyprlock" ];
    bubbles.media.area = "center-left";
  };

  # theme.toml, the same way.
  theme = {
    colors.accent = "#8aadf4";
    layout.mode = "notch";
  };
};
```

The option names are the ones in the [configuration](configuration.md) pages: `[module.idle]` becomes `module.idle`. `mochi config init --print` shows every option with its default.

`home-manager switch` checks both files with `mochid config check`, so a typo fails the switch with the same error the daemon would give, instead of breaking the shell. When they change, the switch runs `mochi reload`; the shell doesn't restart.

The module starts `mochid` as a user service with `wayland.systemd.target`, which is `graphical-session.target` unless you changed it. Set `programs.mochi.systemd.target` to use another target, or `programs.mochi.systemd.enable = false` to start it yourself.

If Mochi already wrote a `config.toml` or `theme.toml`, home-manager refuses to replace it. Move the file away, or keep it with home-manager's `-b backup`.

### NixOS

Import `inputs.mochi.nixosModules.default`, then:

```nix
programs.mochi.enable = true;
```

This installs Mochi for every user and starts it with each user's `graphical-session.target`. Each user's settings stay in `~/.config/mochi`, or in the home-manager module. Use one module or the other, not both.

### Recording

The package brings gpu-screen-recorder, which the capture module records with. With home-manager, setting `module.capture.recorder` in `settings` leaves it out. To record a region or a whole screen, gpu-screen-recorder also needs a helper with extra permissions, which only the system can install. The NixOS module enables it; set `programs.mochi.recording.enable = false` to skip it. With home-manager alone, add this to your NixOS configuration:

```nix
programs.gpu-screen-recorder.enable = true;
```

### The overlay

`inputs.mochi.overlays.default` adds `pkgs.mochi`, for setups that list packages themselves.

## Other systems

```sh
cargo build --release
sudo install -m755 target/release/mochid target/release/mochi /usr/bin/
install -Dm644 systemd/mochid.service ~/.config/systemd/user/mochid.service
systemctl --user enable --now mochid
```

`mochid` needs Quickshell 0.3.1 in its `PATH`, and refuses to start with another version.

## The session

The unit starts with `graphical-session.target`. Session managers such as uwsm start that target and give the user manager `WAYLAND_DISPLAY`. Without one, start `mochid` from your compositor's autostart instead; it doesn't need systemd.

Logs are in `journalctl --user -u mochid`. `MOCHI_LOG=debug` shows everything the daemon decides.

## Compositors

Workspaces come from the standard `ext-workspace-v1` protocol, so the workspace indicator works on any compositor that supports it. The focused monitor comes from the focused window, through `wlr-foreign-toplevel-management`; on Hyprland, its event socket makes that exact. `mochi status` shows what the daemon found.
