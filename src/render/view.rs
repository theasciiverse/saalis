//! The camera: world metres to cells, half-block pixels and Braille dots.
//!
//! A cell is `aspect` times as tall as it is wide (measured from the
//! terminal where it says, 2 otherwise), so circles stay round in any font.

use crate::sim::v2::{v2, V2};
use ratatui::layout::Rect;

/// Measured cell height / width, or 2 (Glider's `cell_aspect`).
pub fn cell_aspect() -> f32 {
    match ratatui::crossterm::terminal::window_size() {
        Ok(w) if w.width > 0 && w.height > 0 && w.columns > 0 && w.rows > 0 => {
            let a = (w.height as f32 / w.rows as f32) / (w.width as f32 / w.columns as f32);
            if (1.2..3.5).contains(&a) { a } else { 2.0 }
        }
        _ => 2.0,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub area: Rect,
    pub centre: V2,
    /// metres per cell column
    pub scale: f32,
    pub aspect: f32,
}

impl View {
    /// The field shows about 1500 m across, within limits, so a small
    /// terminal still sees gun range and a big one sees more.
    pub fn new(area: Rect, centre: V2, aspect: f32) -> View {
        let scale = (1500.0 / area.width.max(1) as f32).clamp(12.0, 26.0);
        View { area, centre, scale, aspect }
    }

    pub fn row_m(&self) -> f32 {
        self.scale * self.aspect
    }

    /// A world point as fractional cell coordinates within the area.
    pub fn cell_f(&self, p: V2) -> (f32, f32) {
        let d = p - self.centre;
        (self.area.width as f32 / 2.0 + d.x / self.scale, self.area.height as f32 / 2.0 + d.y / self.row_m())
    }

    /// A world point as a cell within the area, if it is inside.
    pub fn cell(&self, p: V2) -> Option<(u16, u16)> {
        let (x, y) = self.cell_f(p);
        (x >= 0.0 && y >= 0.0 && x < self.area.width as f32 && y < self.area.height as f32).then_some((x as u16, y as u16))
    }

    /// Half-block pixel coordinates (x = column, y = half rows).
    pub fn pixel(&self, p: V2) -> (i32, i32) {
        let (x, y) = self.cell_f(p);
        (x.floor() as i32, (y * 2.0).floor() as i32)
    }

    /// The world point at the centre of half-block pixel (x, y).
    pub fn pixel_world(&self, x: i32, y: i32) -> V2 {
        let cx = x as f32 + 0.5 - self.area.width as f32 / 2.0;
        let cy = (y as f32 + 0.5) / 2.0 - self.area.height as f32 / 2.0;
        self.centre + v2(cx * self.scale, cy * self.row_m())
    }

    /// Braille dot coordinates.
    pub fn dot(&self, p: V2) -> (i32, i32) {
        let (x, y) = self.cell_f(p);
        ((x * 2.0).floor() as i32, (y * 4.0).floor() as i32)
    }

    /// Metres per half-block pixel, across and down.
    pub fn pixel_m(&self) -> (f32, f32) {
        (self.scale, self.row_m() / 2.0)
    }

    /// Whether a circle of radius `r` around `p` touches the area.
    pub fn sees(&self, p: V2, r: f32) -> bool {
        let d = p - self.centre;
        d.x.abs() - r < self.area.width as f32 / 2.0 * self.scale && d.y.abs() - r < self.area.height as f32 / 2.0 * self.row_m()
    }
}
