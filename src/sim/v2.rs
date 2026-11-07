//! Plane vectors. World units are metres; +x is east, +y is south (down the
//! screen), so headings read like a compass turned to the screen: 0 points
//! north (up), angles grow clockwise.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub const ZERO: V2 = v2(0.0, 0.0);

    /// Unit vector of a heading (0 = up, clockwise).
    pub fn from_heading(h: f32) -> V2 {
        v2(h.sin(), -h.cos())
    }

    /// Heading of this vector (0 = up, clockwise), in -PI..PI.
    pub fn heading(self) -> f32 {
        self.x.atan2(-self.y)
    }

    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 { self * (1.0 / l) } else { V2::ZERO }
    }

    /// The vector turned a quarter clockwise: a ship's right hand.
    pub fn right(self) -> V2 {
        v2(-self.y, self.x)
    }

    /// Moves toward `to` by at most `step`.
    pub fn approach(self, to: V2, step: f32) -> V2 {
        let d = to - self;
        let l = d.len();
        if l <= step { to } else { self + d * (step / l) }
    }
}

/// Smallest signed turn from heading `a` to heading `b`, in -PI..PI.
pub fn turn_to(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        *self = *self + o;
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        *self = *self - o;
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        v2(self.x * k, self.y * k)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn headings_are_a_compass_on_the_screen() {
        let n = V2::from_heading(0.0);
        assert!((n.y + 1.0).abs() < 1e-6, "0 points up the screen");
        let e = V2::from_heading(FRAC_PI_2);
        assert!((e.x - 1.0).abs() < 1e-6, "a quarter turn clockwise points right");
        assert!((e.heading() - FRAC_PI_2).abs() < 1e-6);
        assert!((n.right().x - 1.0).abs() < 1e-6, "facing up, the right hand is east");
    }

    #[test]
    fn turn_to_takes_the_short_way() {
        assert!((turn_to(0.1, -0.1) + 0.2).abs() < 1e-5);
        assert!((turn_to(PI - 0.1, -PI + 0.1) - 0.2).abs() < 1e-5);
    }
}
