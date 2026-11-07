//! Pilots: the steering behaviour behind every ship that isn't flown by a
//! person, and the `--demo` autopilot (the same brain at a high skill).
//!
//! Each brain has a skill (0..1: how well it leads its aim and how soon it
//! pulls the trigger) and a morale (the hull fraction at which it runs, as in
//! Ascii Sector's Timid/Confident/Fanatical). Modes: attack (lead the
//! target, fire when lined up), break (overshot at close range: peel away and
//! come round again) and flee (run until out of range, then leave).

use super::rng::Rng;
use super::ship::{Intent, Ship};
use super::v2::{turn_to, V2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Attack,
    Break { heading: f32 },
    Flee,
}

#[derive(Clone, Debug)]
pub struct Brain {
    pub target: Option<u32>,
    pub mode: Mode,
    /// seconds left in a break
    pub timer: f32,
    pub skill: f32,
    /// hull fraction below which the pilot runs; 0 never runs
    pub flee_at: f32,
    /// the aim error this pilot carries, drifting
    jitter: V2,
    jitter_t: f32,
    /// it has said on the comms that it is running
    pub announced: bool,
}

impl Brain {
    pub fn new(skill: f32, flee_at: f32) -> Brain {
        Brain { target: None, mode: Mode::Attack, timer: 0.0, skill: skill.clamp(0.0, 1.0), flee_at, jitter: V2::ZERO, jitter_t: 0.0, announced: false }
    }
}

/// An obstacle to steer round: a rock or the station.
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub pos: V2,
    pub radius: f32,
}

/// Where to point so a shot of `shot_speed` meets a target at `tp` moving at
/// `tv`, fired from a ship at `p` moving at `v` (shots keep the ship's speed).
pub fn lead(p: V2, v: V2, tp: V2, tv: V2, shot_speed: f32) -> V2 {
    let rel_v = tv - v;
    let mut t = (tp - p).len() / shot_speed;
    for _ in 0..3 {
        t = ((tp + rel_v * t) - p).len() / shot_speed;
    }
    tp + rel_v * t
}

/// Turn intent toward a heading: proportional, saturating.
fn steer(me: &Ship, want: f32) -> f32 {
    (turn_to(me.heading, want) * 4.0).clamp(-1.0, 1.0)
}

/// Steers round whatever lies close ahead, if anything does.
fn avoid(me: &Ship, obstacles: &[Obstacle]) -> Option<f32> {
    let fwd = me.forward();
    let right = fwd.right();
    let look = 350.0 + me.speed() * 1.2;
    obstacles
        .iter()
        .filter_map(|o| {
            let rel = o.pos - me.pos;
            let ahead = rel.dot(fwd);
            let side = rel.dot(right);
            let clear = o.radius + me.class.radius + 60.0;
            (ahead > -o.radius && ahead < look + o.radius && side.abs() < clear).then_some((ahead, side))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, side)| if side > 0.0 { -1.0 } else { 1.0 })
}

/// One tick of thought for `me`, among `ships`.
pub fn think(brain: &mut Brain, me: &Ship, ships: &[Ship], obstacles: &[Obstacle], rng: &mut Rng, dt: f32) -> Intent {
    let target = brain.target.and_then(|id| ships.iter().find(|s| s.id == id && s.alive));
    if me.hull_frac() < brain.flee_at && brain.mode != Mode::Flee {
        brain.mode = Mode::Flee;
    }
    // the aim error wanders; good pilots carry less of it
    brain.jitter_t -= dt;
    if brain.jitter_t <= 0.0 {
        let err = (1.0 - brain.skill) * 90.0;
        brain.jitter = V2 { x: rng.range(-err, err), y: rng.range(-err, err) };
        brain.jitter_t = rng.range(0.4, 1.2);
    }
    let mut it = Intent::default();
    let Some(t) = target else {
        // nothing to fight: cruise on, gently
        it.set_speed = Some(me.class.max_speed * 0.5);
        if let Some(turn) = avoid(me, obstacles) {
            it.turn = turn;
        }
        return it;
    };
    let to = t.pos - me.pos;
    let dist = to.len();
    match brain.mode {
        Mode::Flee => {
            it.turn = steer(me, (-to).heading());
            it.set_speed = Some(me.class.max_speed);
            it.burner = true;
        }
        Mode::Break { heading } => {
            it.turn = steer(me, heading);
            it.set_speed = Some(me.class.max_speed);
            it.burner = brain.skill > 0.5;
            brain.timer -= dt;
            if brain.timer <= 0.0 {
                brain.mode = Mode::Attack;
            }
        }
        Mode::Attack => {
            let gun = me.class.guns[0];
            let aim = lead(me.pos, me.vel, t.pos, t.vel, gun.speed) + brain.jitter;
            let want = (aim - me.pos).heading();
            it.turn = steer(me, want);
            let err = turn_to(me.heading, want).abs();
            let closing = (t.vel - me.vel).dot(to.norm()) < 0.0;
            it.set_speed = Some(if dist > 900.0 { me.class.max_speed } else { t.speed().clamp(me.class.max_speed * 0.5, me.class.max_speed) });
            it.burner = dist > 2500.0 && brain.skill > 0.4;
            it.fire = err < 0.06 + (1.0 - brain.skill) * 0.08 && dist < gun.range * 0.9;
            if dist < 160.0 + me.class.radius && closing {
                let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
                brain.mode = Mode::Break { heading: me.heading + side * rng.range(1.4, 2.2) };
                brain.timer = rng.range(1.0, 2.0);
            }
        }
    }
    if let Some(turn) = avoid(me, obstacles) {
        it.turn = turn;
        it.fire = false;
    }
    it
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ship::{Side, KARPPA, TIIRA};
    use crate::sim::v2::v2;

    #[test]
    fn a_still_target_is_aimed_at_directly() {
        let p = lead(V2::ZERO, V2::ZERO, v2(0.0, -1000.0), V2::ZERO, 1000.0);
        assert!((p.x).abs() < 1e-3 && (p.y + 1000.0).abs() < 1e-3);
    }

    #[test]
    fn a_crossing_target_is_led() {
        let p = lead(V2::ZERO, V2::ZERO, v2(0.0, -1000.0), v2(200.0, 0.0), 1000.0);
        assert!(p.x > 150.0, "aim ahead of where it is going");
    }

    #[test]
    fn a_pilot_turns_toward_its_target_and_fires_when_lined_up() {
        let me = Ship::new(2, &KARPPA, Side::Pirate, "k", V2::ZERO, 0.0);
        let you = Ship::new(1, &TIIRA, Side::Player, "you", v2(0.0, -600.0), 0.0);
        let mut you = you;
        you.vel = V2::ZERO;
        let mut me = me;
        me.vel = V2::ZERO;
        let mut b = Brain::new(1.0, 0.0);
        b.target = Some(1);
        let ships = vec![you.clone(), me.clone()];
        let it = think(&mut b, &me, &ships, &[], &mut Rng::new(3), 1.0 / 60.0);
        assert!(it.fire, "dead ahead and in range");
        me.heading = 1.0;
        let it = think(&mut b, &me, &ships, &[], &mut Rng::new(3), 1.0 / 60.0);
        assert!(it.turn < 0.0 && !it.fire, "turned away: swing back left, hold fire");
    }

    #[test]
    fn a_badly_hurt_pilot_runs() {
        let mut me = Ship::new(2, &KARPPA, Side::Pirate, "k", V2::ZERO, 0.0);
        me.hull = 1.0;
        let you = Ship::new(1, &TIIRA, Side::Player, "you", v2(0.0, -600.0), 0.0);
        let mut b = Brain::new(0.5, 0.4);
        b.target = Some(1);
        let it = think(&mut b, &me, &[you], &[], &mut Rng::new(3), 1.0 / 60.0);
        assert_eq!(b.mode, Mode::Flee);
        assert!(it.burner && !it.fire);
    }
}
