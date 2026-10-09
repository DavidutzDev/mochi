//! Screens in a bento: `screen-1` is the largest, `screen-2` the next, so
//! widgets land on the same kind of screen on another machine, whatever
//! its outputs are called.

use std::time::Duration;

/// How long to wait for the compositor to name the screens.
const WAIT: Duration = Duration::from_secs(2);

/// The screen a role names, from 0: `screen-2` is 1.
pub fn parse_role(role: &str) -> Option<usize> {
    let number: usize = role.strip_prefix("screen-")?.parse().ok()?;
    number.checked_sub(1)
}

pub fn role(index: usize) -> String {
    format!("screen-{}", index + 1)
}

/// Output names, largest first, then by name.
pub fn rank(mut outputs: Vec<(String, u32, u32)>) -> Vec<String> {
    outputs.sort_by(|a, b| {
        let area = |(_, width, height): &(String, u32, u32)| u64::from(*width) * u64::from(*height);
        area(b).cmp(&area(a)).then_with(|| a.0.cmp(&b.0))
    });
    outputs.into_iter().map(|(name, _, _)| name).collect()
}

/// The screens connected now, largest first. Empty without a compositor
/// that says.
pub fn connected() -> Vec<String> {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    runtime.block_on(async {
        let compositor = mochi_core::compositor::connect();
        let mut state = compositor.subscribe();
        let sized = |state: &mochi_core::compositor::State| {
            !state.outputs.is_empty() && state.outputs.iter().all(|output| output.width > 0)
        };
        let _ = tokio::time::timeout(WAIT, async {
            while !sized(&state.borrow()) {
                if state.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
        let outputs = compositor
            .state()
            .outputs
            .into_iter()
            .filter(|output| !mochi_core::compositor::is_virtual(&output.name))
            .map(|output| (output.name, output.width, output.height))
            .collect();
        rank(outputs)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_largest_screen_comes_first() {
        let ranked = rank(vec![
            ("eDP-1".into(), 1920, 1200),
            ("DP-3".into(), 2560, 1440),
            ("DP-2".into(), 2560, 1440),
        ]);
        assert_eq!(ranked, ["DP-2", "DP-3", "eDP-1"]);
        assert_eq!(parse_role("screen-1"), Some(0));
        assert_eq!(parse_role(&role(2)), Some(2));
        assert_eq!(parse_role("screen-0"), None);
        assert_eq!(parse_role("DP-3"), None);
    }
}
