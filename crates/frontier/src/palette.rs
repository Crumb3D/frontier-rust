//! Colours sampled from the reference screenshots in `assets/` and the DOS stills.

pub type Rgb = [u8; 3];

pub const BLACK: Rgb = [0, 0, 0];
pub const WHITE: Rgb = [255, 255, 255];

// Station and status screens, sampled from DOSBox captures of the original.
pub const STATION_BG: Rgb = [93, 77, 158];
pub const CREAM: Rgb = [255, 239, 203];
pub const LILAC: Rgb = [125, 117, 203];
pub const STATUS_BG: Rgb = [77, 77, 142];
pub const PROFILE_BG: Rgb = [77, 77, 77];
pub const ROSTER_BG: Rgb = [142, 77, 12];
pub const ROSTER_RED: Rgb = [227, 0, 0];
/// Square station button: highlight corner, lit edge, face, shaded edge.
pub const BTN: [Rgb; 4] = [[227, 227, 227], [162, 162, 162], [97, 97, 97], [65, 65, 65]];
pub const ARROW_FACE: Rgb = [130, 130, 130];
pub const TITLE_NAVY: Rgb = [12, 12, 93];

pub const GREY_136: Rgb = [136, 136, 136];
pub const GREY_102: Rgb = [102, 102, 102];

// DOS original (PIC002.GRF): console text, HUD and the Merlin sky.
pub const CON_ORANGE: Rgb = [224, 116, 0];
pub const CON_GREEN: Rgb = [0, 224, 0];
pub const HUD_WHITE: Rgb = [252, 252, 252];
pub const DOS_GROUND: Rgb = [44, 44, 44];
pub const DOME_PINK: Rgb = [236, 140, 140];
pub const WINDOW_YELLOW: Rgb = [236, 236, 12];
/// Sky colour bands from the zenith down to the horizon.
pub const SKY_MERLIN: [Rgb; 5] =
    [[124, 12, 140], [156, 44, 140], [188, 76, 140], [220, 108, 140], [252, 140, 140]];
/// Ground haze just below the horizon: a one-row line then a band.
pub const HAZE_MERLIN: [Rgb; 2] = [[236, 140, 140], [188, 108, 108]];
/// Aster's rings from space: a grey band edged in red.
pub const RING_ASTER: [Rgb; 2] = [[76, 76, 76], [172, 76, 76]];
/// Merlin's seas from space (its land colour is the planet colour).
pub const MERLIN_SEA: Rgb = [124, 44, 108];

pub fn scale(c: Rgb, f: f64) -> Rgb {
    let f = f.max(0.0);
    [
        (c[0] as f64 * f).min(255.0) as u8,
        (c[1] as f64 * f).min(255.0) as u8,
        (c[2] as f64 * f).min(255.0) as u8,
    ]
}

pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f64 + (b[0] as f64 - a[0] as f64) * t) as u8,
        (a[1] as f64 + (b[1] as f64 - a[1] as f64) * t) as u8,
        (a[2] as f64 + (b[2] as f64 - a[2] as f64) * t) as u8,
    ]
}

/// Snap to the DOS version's 3D palette: 4 bits per channel, stored as n*16+12.
pub fn dos(c: Rgb) -> Rgb {
    let q = |v: u8| ((v as i32 - 4).max(0) / 16).min(15) as u8 * 16 + 12;
    [q(c[0]), q(c[1]), q(c[2])]
}
