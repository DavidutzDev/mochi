//! Color temperature as gamma ramps: how much of each channel a warmer
//! white keeps.

/// Daylight: the screen as it is.
pub const NEUTRAL: u32 = 6500;
pub const WARMEST: u32 = 1000;

/// The white of a black body at `kelvin`, from Tanner Helland's fit, as
/// red, green and blue from 0 to 1.
fn black_body(kelvin: u32) -> [f64; 3] {
    let t = f64::from(kelvin.clamp(WARMEST, 40_000)) / 100.0;
    let red = if t <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (t - 60.0).powf(-0.133_204_759_2)
    };
    let green = if t <= 66.0 {
        99.470_802_586_1 * t.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (t - 60.0).powf(-0.075_514_849_2)
    };
    let blue = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (t - 10.0).ln() - 305.044_792_730_7
    };
    [red, green, blue].map(|channel| (channel / 255.0).clamp(0.0, 1.0))
}

/// What each channel is scaled by at `kelvin`, so that [`NEUTRAL`] leaves
/// the screen alone.
pub fn white_point(kelvin: u32) -> [f64; 3] {
    let neutral = black_body(NEUTRAL);
    let warm = black_body(kelvin.min(NEUTRAL));
    [0, 1, 2].map(|channel| (warm[channel] / neutral[channel]).clamp(0.0, 1.0))
}

/// A gamma table of `size` steps a channel, as the protocol takes it: every
/// red step, then every green one, then every blue one.
pub fn ramps(size: usize, kelvin: u32) -> Vec<u16> {
    let white = white_point(kelvin);
    let mut table = Vec::with_capacity(size * 3);
    for scale in white {
        for step in 0..size {
            let level = if size > 1 {
                step as f64 / (size - 1) as f64
            } else {
                1.0
            };
            table.push((level * scale * f64::from(u16::MAX)).round() as u16);
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daylight_leaves_the_screen_alone() {
        assert_eq!(white_point(NEUTRAL), [1.0, 1.0, 1.0]);
        assert_eq!(white_point(9000), [1.0, 1.0, 1.0]);
        let table = ramps(4, NEUTRAL);
        assert_eq!(
            table,
            [
                0, 21845, 43690, 65535, 0, 21845, 43690, 65535, 0, 21845, 43690, 65535
            ]
        );
    }

    #[test]
    fn warmer_takes_blue_first() {
        let [red, green, blue] = white_point(4000);
        assert_eq!(red, 1.0);
        assert!(blue < green && green < 1.0, "{green} {blue}");
        let [_, _, warmest] = white_point(WARMEST);
        assert!(warmest < blue);
        let table = ramps(256, 3000);
        assert_eq!(table.len(), 768);
        assert_eq!(table[255], u16::MAX);
        assert!(table[767] < table[511]);
    }
}
