# Installing

## Nix

The flake has the `mochi` package, with both binaries, the systemd unit and the Quickshell version `mochid` expects. It also has a home-manager module, a NixOS module and an overlay. Add it to your flake's inputs, pinned to a release:

```nix
mochi = {
  url = "github:DavidutzDev/mochi/v0.0.7";
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

## Arch Linux

The repository has two packages in `packaging/arch`, with the same files:

- `mochi` builds the latest release.
- `mochi-git` builds the latest commit on `main`, and conflicts with `mochi`.

They aren't on the AUR yet. Until they are, build one from the repository with `makepkg`, which needs `base-devel` and `git`:

```sh
git clone https://github.com/DavidutzDev/mochi.git
cd mochi/packaging/arch/mochi-git    # or mochi, for the latest release
makepkg -si
```

`makepkg -si` installs the missing dependencies, builds the package and installs it with pacman. To update `mochi-git`, run `makepkg -si` again in the same folder: it fetches the latest commit itself. For `mochi`, `git pull` first, to get the `PKGBUILD` of the newest release.

Once they're on the AUR, `paru -S mochi` or `yay -S mochi` does the same.

Both depend on `quickshell`, `gpu-screen-recorder` and `inter-font` from the official repositories, and download the Material Symbols icon font from Google's repository, pinned to one commit. The build runs the test suite. They install `mochid`, `mochi`, the systemd user unit and `mochi-share-picker`. Optional dependencies:

- `wl-clipboard`, to copy screenshots.
- `satty`, the default screenshot editor.
- `uwsm`, to start apps from the launcher as units of their own. Without it, the launcher uses `systemd-run`.
- `xdg-desktop-portal-hyprland`, for the [share](modules/share.md) module.

Then start the shell with your session:

```sh
systemctl --user enable --now mochid
```

The unit needs a session manager such as uwsm; see [the session](#the-session). On the first start, Mochi writes a commented `~/.config/mochi/config.toml`. `mochi config init --print` shows every option with its default.

Arch's `gpu-screen-recorder` package already gives its helper the permissions it needs to record a region or a whole screen, so recording needs no more setup.

To use the share module's picker, point the portal at it in `~/.config/hypr/xdph.conf`, then restart the portal with `systemctl --user restart xdg-desktop-portal-hyprland`:

```
screencopy {
    custom_picker_binary = /usr/bin/mochi-share-picker
}
```

The `PKGBUILD`s live in `packaging/arch`. When Quickshell in the official repositories moves past the version `mochid` expects, `mochid` refuses to start until a Mochi release follows it.

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
