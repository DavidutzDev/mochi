//! What the computer is: its distribution, kernel and CPU, when it started,
//! and the compositor Mochi runs in, for the settings' About page and the
//! system info widget. Each reader has a parser of the file's text beside
//! it, for the tests.

/// The distribution's name, like `NixOS 26.05 (Yarara)`, from os-release.
pub fn os() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    os_name(&text)
}

/// `PRETTY_NAME`, or else `NAME`, from os-release's text.
pub fn os_name(os_release: &str) -> Option<String> {
    let field = |key: &str| {
        os_release
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .map(|value| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
    };
    field("PRETTY_NAME").or_else(|| field("NAME"))
}

/// The kernel's release, like `7.2.9-zen1`.
pub fn kernel() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The CPU's model, from `/proc/cpuinfo`.
pub fn cpu() -> Option<String> {
    cpu_model(&std::fs::read_to_string("/proc/cpuinfo").ok()?)
}

/// The first `model name`, `Model` or `Hardware` line of cpuinfo's text,
/// with its runs of spaces made one.
pub fn cpu_model(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            matches!(key.trim(), "model name" | "Model" | "Hardware").then(|| value.trim())
        })
        .map(|name| name.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|name| !name.is_empty())
}

/// When the computer started, in seconds since the epoch.
pub fn booted() -> Option<u64> {
    boot_time(&std::fs::read_to_string("/proc/stat").ok()?)
}

/// The `btime` line of `/proc/stat`'s text.
pub fn boot_time(stat: &str) -> Option<u64> {
    stat.lines()
        .find_map(|line| line.strip_prefix("btime "))
        .and_then(|value| value.trim().parse().ok())
}

/// The compositor, like `Hyprland` or `niri`, from the environment.
pub fn compositor() -> Option<String> {
    compositor_from(|name| std::env::var(name).ok())
}

/// The first desktop `XDG_CURRENT_DESKTOP` names, or else the compositor
/// whose socket's variable is set.
pub fn compositor_from(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    let desktop = var("XDG_CURRENT_DESKTOP").and_then(|desktops| {
        desktops
            .split(':')
            .map(str::trim)
            .find(|desktop| !desktop.is_empty())
            .map(str::to_owned)
    });
    desktop.or_else(|| {
        [
            ("HYPRLAND_INSTANCE_SIGNATURE", "Hyprland"),
            ("NIRI_SOCKET", "niri"),
            ("SWAYSOCK", "sway"),
        ]
        .into_iter()
        .find(|(name, _)| var(name).is_some_and(|value| !value.is_empty()))
        .map(|(_, compositor)| compositor.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_distribution() {
        let text = "NAME=NixOS\nID=nixos\nPRETTY_NAME=\"NixOS 26.05 (Yarara)\"\n";
        assert_eq!(os_name(text).as_deref(), Some("NixOS 26.05 (Yarara)"));
        assert_eq!(
            os_name("NAME=\"Arch Linux\"\n").as_deref(),
            Some("Arch Linux")
        );
        assert_eq!(os_name("ID=void\n"), None);
    }

    #[test]
    fn reads_the_cpu() {
        let text = "processor\t: 0\nmodel name\t: AMD Ryzen 7  7840U   w/ Radeon\n";
        assert_eq!(
            cpu_model(text).as_deref(),
            Some("AMD Ryzen 7 7840U w/ Radeon")
        );
        assert_eq!(
            cpu_model("Hardware\t: BCM2835\n").as_deref(),
            Some("BCM2835")
        );
        assert_eq!(cpu_model("processor\t: 0\n"), None);
    }

    #[test]
    fn reads_the_boot_time() {
        let stat = "cpu  1 2 3 4\nintr 5\nbtime 1791540000\nprocesses 9\n";
        assert_eq!(boot_time(stat), Some(1_791_540_000));
        assert_eq!(boot_time("cpu 1 2 3\n"), None);
    }

    #[test]
    fn names_the_compositor() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        assert_eq!(
            compositor_from(env(&[("XDG_CURRENT_DESKTOP", "Hyprland")])).as_deref(),
            Some("Hyprland")
        );
        assert_eq!(
            compositor_from(env(&[("XDG_CURRENT_DESKTOP", "niri:GNOME")])).as_deref(),
            Some("niri")
        );
        // Without the desktop's name, the socket says which.
        assert_eq!(
            compositor_from(env(&[("SWAYSOCK", "/run/user/1000/sway-ipc.sock")])).as_deref(),
            Some("sway")
        );
        assert_eq!(compositor_from(env(&[("XDG_CURRENT_DESKTOP", "")])), None);
    }
}
