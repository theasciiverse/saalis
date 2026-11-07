//! Colours and ramps. Truecolor throughout.

use ratatui::style::Color;

pub type Rgb = (u8, u8, u8);

/// Deep space, a little blue.
pub const SPACE: Color = Color::Rgb(5, 7, 14);
pub const CHROME_BG: Color = Color::Rgb(9, 13, 22);
pub const FRAME: Color = Color::Rgb(52, 74, 96);
pub const DIM: Color = Color::Rgb(96, 116, 136);
pub const TEXT: Color = Color::Rgb(206, 220, 232);
pub const TRACE: Color = Color::Rgb(34, 48, 62);
/// Box-drawing wireframes: schematics, the dock, the logo.
pub const WIRE: Color = Color::Rgb(110, 200, 220);

/// Gauges run from danger to full.
pub const GAUGE: [Rgb; 3] = [(230, 70, 60), (240, 190, 80), (110, 220, 170)];
pub const SHIELD: Rgb = (120, 200, 255);
pub const ENERGY: Rgb = (255, 214, 110);
pub const BURN: Rgb = (255, 150, 70);

pub const STAR: [Color; 3] = [Color::Rgb(54, 60, 88), Color::Rgb(110, 118, 160), Color::Rgb(210, 216, 250)];

/// The player's colours: ice and cyan.
pub const PLAYER: Rgb = (190, 236, 255);
/// Gunfire has colours of its own, so a shot never reads as a ship's nose
/// line (David, 2026-11-07): lime for yours, magenta for theirs. Nothing
/// else uses either.
pub const PLAYER_SHOT: Rgb = (160, 255, 110);
pub const PIRATE_SHOT: Rgb = (255, 70, 210);
pub const PIRATE: Rgb = (240, 80, 80);
/// The selected target's brackets and arrow.
pub const TARGET: Color = Color::Rgb(255, 196, 80);
pub const STATION: Rgb = (150, 170, 190);
pub const WINDOW: Rgb = (255, 210, 130);
pub const ROCK: [Rgb; 3] = [(120, 108, 96), (104, 100, 98), (130, 118, 100)];
/// Drive lanes: their gates, the map's routes.
pub const LANE: Rgb = (176, 140, 255);
/// Each faction's colour, for its ships, its blips and its voice.
pub fn faction(f: crate::sim::faction::Faction) -> Rgb {
    use crate::sim::faction::Faction::*;
    match f {
        Coop => (120, 220, 140),
        Crown => (232, 200, 96),
        Traders => (110, 210, 215),
        Inner => (150, 170, 255),
        Pirates => PIRATE,
    }
}

pub const TRAFFIC: Color = Color::Rgb(130, 220, 200);

pub fn rgb(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    let m = |x: u8| (x as f32 * k).clamp(0.0, 255.0) as u8;
    (m(c.0), m(c.1), m(c.2))
}

pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// A point along a multi-stop ramp, t in [0, 1].
pub fn ramp(stops: &[Rgb], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (t.floor() as usize).min(stops.len() - 2);
    rgb(lerp(stops[i], stops[i + 1], t - i as f32))
}
