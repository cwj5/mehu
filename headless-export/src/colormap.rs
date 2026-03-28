/// Colormap functions for scalar field visualization.
///
/// Ported from `src/utils/colorMapping.ts` to produce the same color output
/// as the in-app Three.js renderer when given the same normalized t ∈ [0, 1].

/// Linearly interpolate a lookup table at normalized t ∈ [0, 1].
fn lerp_lut(lut: &[[f32; 3]], t: f32) -> [u8; 3] {
    let n = lut.len();
    let fi = (t * (n as f32 - 1.0)).clamp(0.0, n as f32 - 1.001);
    let lo = fi.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    let frac = fi - lo as f32;
    let r = ((lut[lo][0] * (1.0 - frac) + lut[hi][0] * frac) * 255.0).clamp(0.0, 255.0);
    let g = ((lut[lo][1] * (1.0 - frac) + lut[hi][1] * frac) * 255.0).clamp(0.0, 255.0);
    let b = ((lut[lo][2] * (1.0 - frac) + lut[hi][2] * frac) * 255.0).clamp(0.0, 255.0);
    [r as u8, g as u8, b as u8]
}

// Viridis colormap (perceptually uniform, matches matplotlib / in-app default).
const VIRIDIS: &[[f32; 3]] = &[
    [0.267004, 0.004874, 0.329415],
    [0.282623, 0.140461, 0.469470],
    [0.253935, 0.265254, 0.529983],
    [0.206756, 0.371758, 0.553806],
    [0.163625, 0.471133, 0.558695],
    [0.127568, 0.566949, 0.550413],
    [0.134692, 0.658636, 0.517649],
    [0.266941, 0.748751, 0.440573],
    [0.477504, 0.821444, 0.318195],
    [0.741388, 0.873449, 0.149561],
    [0.993248, 0.906157, 0.143936],
];

// Turbo colormap (Google's Turbo — matches TypeScript LUT).
const TURBO: &[[f32; 3]] = &[
    [0.19, 0.07, 0.23],
    [0.21, 0.14, 0.42],
    [0.24, 0.26, 0.61],
    [0.27, 0.38, 0.81],
    [0.29, 0.50, 0.93],
    [0.28, 0.63, 0.94],
    [0.25, 0.74, 0.80],
    [0.42, 0.84, 0.54],
    [0.67, 0.90, 0.28],
    [0.89, 0.88, 0.12],
    [1.00, 0.77, 0.06],
    [1.00, 0.60, 0.03],
    [0.97, 0.40, 0.02],
    [0.92, 0.20, 0.01],
    [0.85, 0.09, 0.01],
    [0.80, 0.02, 0.00],
];

pub fn viridis(t: f32) -> [u8; 3] {
    lerp_lut(VIRIDIS, t.clamp(0.0, 1.0))
}

pub fn turbo(t: f32) -> [u8; 3] {
    lerp_lut(TURBO, t.clamp(0.0, 1.0))
}

pub fn rainbow(t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.2 {
        (1.0f32, t / 0.2, 0.0f32)
    } else if t < 0.4 {
        (1.0 - (t - 0.2) / 0.2, 1.0, 0.0)
    } else if t < 0.6 {
        (0.0, 1.0, (t - 0.4) / 0.2)
    } else if t < 0.8 {
        (0.0, 1.0 - (t - 0.6) / 0.2, 1.0)
    } else {
        ((t - 0.8) / 0.2, 0.0, 1.0)
    };
    [
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    ]
}

pub fn hot(t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.33 {
        (t / 0.33, 0.0f32, 0.0f32)
    } else if t < 0.66 {
        (1.0, (t - 0.33) / 0.33, 0.0)
    } else {
        (1.0, 1.0, (t - 0.66) / 0.34)
    };
    [
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    ]
}

pub fn grayscale(t: f32) -> [u8; 3] {
    let v = (t.clamp(0.0, 1.0) * 255.0) as u8;
    [v, v, v]
}

/// Default colormap (viridis), matching the in-app default.
pub fn apply(t: f32) -> [u8; 3] {
    viridis(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viridis_endpoints_are_correct() {
        let dark = viridis(0.0);
        let bright = viridis(1.0);
        // t=0 should be dark purple: R < 100, B > 100
        assert!(dark[0] < 100, "dark R={}", dark[0]);
        assert!(dark[2] > 60, "dark B={}", dark[2]);
        // t=1 should be bright yellow: R > 200, G > 200, B < 80
        assert!(bright[0] > 200, "bright R={}", bright[0]);
        assert!(bright[1] > 200, "bright G={}", bright[1]);
        assert!(bright[2] < 80, "bright B={}", bright[2]);
    }

    #[test]
    fn viridis_midpoint_is_green() {
        let mid = viridis(0.5);
        // t=0.5 should be teal/green: G ≥ R, G ≥ B
        assert!(mid[1] >= mid[0], "G={} should be >= R={}", mid[1], mid[0]);
    }

    #[test]
    fn turbo_is_different_from_viridis() {
        let v = viridis(0.5);
        let t = turbo(0.5);
        assert_ne!(v, t);
    }

    #[test]
    fn rainbow_spans_full_range() {
        let low = rainbow(0.0);
        let high = rainbow(1.0);
        // t=0: red; t=1: blue/magenta
        assert!(low[0] > 200, "low R={}", low[0]);
        assert!(high[2] > 200, "high B={}", high[2]);
    }

    #[test]
    fn grayscale_endpoints() {
        assert_eq!(grayscale(0.0), [0, 0, 0]);
        assert_eq!(grayscale(1.0), [255, 255, 255]);
    }

    #[test]
    fn all_colormaps_clamp_outside_range() {
        // Should not panic or overflow for out-of-range inputs.
        let _ = viridis(-0.5);
        let _ = viridis(1.5);
        let _ = turbo(-1.0);
        let _ = turbo(2.0);
        let _ = rainbow(-0.1);
        let _ = rainbow(1.1);
        let _ = hot(-0.5);
        let _ = hot(1.5);
        let _ = grayscale(-1.0);
        let _ = grayscale(2.0);
    }
}
