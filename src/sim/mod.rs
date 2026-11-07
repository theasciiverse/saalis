//! The simulation: one belt region of the Northern Reach, with Routa station,
//! its rocks, the player and waves of claim-jumpers. No terminal code here
//! (Torched's split), so it runs headless in tests and in `--demo`.

pub mod ai;
pub mod cue;
pub mod rng;
pub mod ship;
pub mod v2;

use ai::{Brain, Obstacle};
use cue::Cue;
use rng::Rng;
use ship::{Class, Intent, Ship, Side, Size, Struck};
use std::collections::HashMap;
use v2::{v2, V2};

pub const PLAYER: u32 = 1;
/// Pirates further than this from the player give up the chase and leave.
const LEAVE_RANGE: f32 = 6500.0;
/// Seconds between one wave gone and the next arriving.
const LULL: f32 = 8.0;
/// Shots this close to the player are heard.
const EARSHOT: f32 = 1600.0;

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: V2,
    pub vel: V2,
    pub owner: u32,
    pub side: Side,
    pub damage: f32,
    /// metres left to fly
    pub left: f32,
}

/// A Braille spark: hit flashes, debris, engine wash.
#[derive(Clone, Copy, Debug)]
pub struct Spark {
    pub pos: V2,
    pub vel: V2,
    pub age: f32,
    pub life: f32,
    pub kind: SparkKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparkKind {
    Shield,
    Armour,
    Hull,
    Debris,
    Rock,
}

/// A half-block fireball.
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub pos: V2,
    pub vel: V2,
    pub radius: f32,
    pub age: f32,
    pub life: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rock {
    pub pos: V2,
    pub radius: f32,
    /// shapes the lumps and the colour
    pub seed: u32,
}

#[derive(Clone, Debug)]
pub struct Station {
    pub name: &'static str,
    pub pos: V2,
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct Comm {
    pub from: String,
    pub text: String,
    pub side: Option<Side>,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Flying,
    /// the player's ship is gone; the field plays on
    Lost,
}

pub struct World {
    pub rng: Rng,
    pub time: f32,
    pub ships: Vec<Ship>,
    brains: HashMap<u32, Brain>,
    pub shots: Vec<Shot>,
    pub sparks: Vec<Spark>,
    pub blasts: Vec<Blast>,
    pub rocks: Vec<Rock>,
    pub station: Station,
    pub comms: Vec<Comm>,
    pub cues: Vec<Cue>,
    /// the player's selected target
    pub target: Option<u32>,
    pub wave: u32,
    pub kills: u32,
    pub score: u32,
    pub phase: Phase,
    /// seconds in the current phase
    pub phase_t: f32,
    /// seconds until the next wave, while none is out
    lull: f32,
    /// the dock crews have said so already this calm
    repair_told: bool,
    next_id: u32,
}

const ARRIVALS: [&str; 6] = [
    "This rock's claimed. Turn round or be salvage.",
    "Nice little hull. We'll take it.",
    "Cargo out the hatch and you might live.",
    "Routa can't help you this far up.",
    "Fresh saalis, everyone. Go.",
    "Nobody flies the high orbits for free.",
];
const RUNNING: [&str; 4] = ["Not worth it. I'm out.", "Breaking off! Breaking off!", "You'll keep.", "Too hot. Going dark."];
const KILLED: [&str; 3] = ["Bounty logged: {}.", "Confirmed. One {} less.", "Good kill. {} down."];

impl World {
    pub fn new(seed: u64) -> World {
        let mut rng = Rng::new(seed);
        let station = Station { name: "Routa", pos: V2::ZERO, radius: 260.0 };
        let rocks = scatter_rocks(&mut rng);
        let mut w = World {
            rng,
            time: 0.0,
            ships: Vec::new(),
            brains: HashMap::new(),
            shots: Vec::new(),
            sparks: Vec::new(),
            blasts: Vec::new(),
            rocks,
            station,
            comms: Vec::new(),
            cues: Vec::new(),
            target: None,
            wave: 0,
            kills: 0,
            score: 0,
            phase: Phase::Flying,
            phase_t: 0.0,
            lull: 4.0,
            repair_told: false,
            next_id: PLAYER + 1,
        };
        w.launch();
        w.say_traffic("Clear of the dock. Patrol space is yours.");
        w
    }

    /// Puts the player's ship outside the dock.
    fn launch(&mut self) {
        self.ships.retain(|s| s.id != PLAYER);
        let pos = self.station.pos + v2(0.0, -(self.station.radius + 260.0));
        let mut s = Ship::new(PLAYER, &ship::TIIRA, Side::Player, "you", pos, 0.0);
        s.set_speed = 120.0;
        s.vel = V2::from_heading(0.0) * 120.0;
        self.ships.insert(0, s);
        self.phase = Phase::Flying;
        self.phase_t = 0.0;
    }

    /// After a loss: a spare hull, the field cleared, the waves go on.
    pub fn relaunch(&mut self) {
        self.ships.retain(|s| s.side == Side::Player);
        self.brains.clear();
        self.shots.clear();
        self.target = None;
        self.lull = LULL;
        self.launch();
        self.say_traffic("Spare hull's warm. Try to bring this one back.");
        log::info!("relaunch wave={} kills={}", self.wave, self.kills);
    }

    pub fn player(&self) -> Option<&Ship> {
        self.ships.iter().find(|s| s.id == PLAYER && s.alive)
    }

    pub fn ship(&self, id: u32) -> Option<&Ship> {
        self.ships.iter().find(|s| s.id == id && s.alive)
    }

    pub fn hostiles(&self) -> impl Iterator<Item = &Ship> {
        self.ships.iter().filter(|s| s.alive && s.side == Side::Pirate)
    }

    /// Whether a ship is running for it.
    pub fn fleeing(&self, id: u32) -> bool {
        self.brains.get(&id).is_some_and(|b| b.mode == ai::Mode::Flee)
    }

    pub fn obstacles(&self) -> Vec<Obstacle> {
        let mut o: Vec<Obstacle> = self.rocks.iter().map(|r| Obstacle { pos: r.pos, radius: r.radius }).collect();
        o.push(Obstacle { pos: self.station.pos, radius: self.station.radius });
        o
    }

    fn say(&mut self, from: impl Into<String>, text: impl Into<String>, side: Option<Side>) {
        let (from, text) = (from.into(), text.into());
        log::info!("comms {from}: {text}");
        self.comms.push(Comm { from, text, side, age: 0.0 });
        if self.comms.len() > 20 {
            self.comms.remove(0);
        }
        self.cues.push(Cue::Comms);
    }

    fn say_traffic(&mut self, text: impl Into<String>) {
        let from = format!("{} Traffic", self.station.name);
        self.say(from, text, None);
    }

    // --- the player's own controls -------------------------------------

    /// T: the next hostile, nearest first.
    pub fn cycle_target(&mut self) {
        let Some(me) = self.player() else { return };
        let mut ids: Vec<(f32, u32)> = self.hostiles().map(|s| ((s.pos - me.pos).len(), s.id)).collect();
        ids.sort_by(|a, b| a.0.total_cmp(&b.0));
        let next = match self.target.and_then(|t| ids.iter().position(|&(_, id)| id == t)) {
            Some(i) => ids.get((i + 1) % ids.len()).map(|x| x.1),
            None => ids.first().map(|x| x.1),
        };
        if next.is_some() && next != self.target {
            self.cues.push(Cue::Target);
        }
        self.target = next;
    }

    /// R: the nearest hostile.
    pub fn nearest_target(&mut self) {
        self.target = None;
        self.cycle_target();
    }

    /// The set speed the player's Home / End / match keys ask for.
    pub fn match_speed(&self) -> Option<f32> {
        self.target.and_then(|t| self.ship(t)).map(|t| t.speed())
    }

    // --- the tick -----------------------------------------------------

    /// One tick: `player` is what the person (or the demo autopilot) asks.
    pub fn update(&mut self, player: &Intent, dt: f32) {
        self.time += dt;
        self.phase_t += dt;
        let obstacles = self.obstacles();
        let player_pos = self.player().map(|p| p.pos);

        // the pilots decide, then everyone flies
        let mut intents = Vec::with_capacity(self.ships.len());
        for s in &self.ships {
            let it = if s.id == PLAYER {
                *player
            } else if let Some(b) = self.brains.get_mut(&s.id) {
                b.target = player_pos.map(|_| PLAYER);
                ai::think(b, s, &self.ships, &obstacles, &mut self.rng, dt)
            } else {
                Intent::default()
            };
            intents.push(it);
        }
        for (i, it) in intents.iter().enumerate() {
            let near = player_pos.is_some_and(|p| (self.ships[i].pos - p).len() < EARSHOT);
            let s = &mut self.ships[i];
            if !s.alive {
                continue;
            }
            if s.fly(it, dt) && s.id == PLAYER {
                self.cues.push(Cue::Burner);
            }
            if it.fire {
                if let Some(g) = s.ready_gun() {
                    let gun = s.class.guns[g];
                    s.cooldown[g] = gun.refire;
                    s.energy -= gun.energy;
                    s.next_gun = (g + 1) % s.class.guns.len();
                    let n = s.class.guns.len() as f32;
                    let side = if n > 1.0 { (g as f32 / (n - 1.0) - 0.5) * s.class.radius } else { 0.0 };
                    let fwd = s.forward();
                    self.shots.push(Shot {
                        pos: s.pos + fwd * s.class.radius + fwd.right() * side,
                        vel: s.vel + fwd * gun.speed,
                        owner: s.id,
                        side: s.side,
                        damage: gun.damage,
                        left: gun.range,
                    });
                    if s.id == PLAYER {
                        self.cues.push(Cue::Gun);
                    } else if near {
                        self.cues.push(Cue::EnemyGun);
                    }
                } else if s.id == PLAYER && s.energy < s.class.guns[0].energy && self.phase_t > 0.2 {
                    self.cues.push(Cue::Empty);
                }
            }
        }

        self.move_shots(dt);
        self.collide(dt);
        self.age_effects(dt);
        self.tidy();
        self.waves(dt);
        self.repair(dt);
        for c in &mut self.comms {
            c.age += dt;
        }
        if self.target.is_some_and(|t| self.ship(t).is_none()) {
            self.target = None;
        }
        if self.target.is_none() && self.hostiles().next().is_some() && self.player().is_some() {
            self.nearest_target();
        }
    }

    fn move_shots(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|sh| {
            let from = sh.pos;
            let step = sh.vel * dt;
            sh.pos += step;
            sh.left -= step.len();
            // ships of the other side
            let hit = self.ships.iter().position(|s| s.alive && s.side != sh.side && seg_hits(from, sh.pos, s.pos, s.class.radius));
            if let Some(i) = hit {
                self.hit(i, sh);
                return false;
            }
            let in_rock = self.rocks.iter().any(|r| (sh.pos - r.pos).len() < r.radius * 0.9) || (sh.pos - self.station.pos).len() < self.station.radius * 0.9;
            if in_rock {
                self.spray(sh.pos, -sh.vel.norm() * 60.0, 3, SparkKind::Rock);
                return false;
            }
            sh.left > 0.0
        });
        self.shots.extend(shots);
    }

    fn hit(&mut self, i: usize, sh: &Shot) {
        let from = (-sh.vel).heading();
        let struck = self.ships[i].take_hit(sh.damage, from);
        let (pos, vel, id) = (sh.pos, self.ships[i].vel, self.ships[i].id);
        let kind = match struck {
            Struck::Shield => SparkKind::Shield,
            Struck::Armour => SparkKind::Armour,
            _ => SparkKind::Hull,
        };
        self.spray(pos, vel, 5, kind);
        if id == PLAYER {
            self.cues.push(if struck == Struck::Shield { Cue::ShieldHit } else { Cue::ArmourHit });
        } else if sh.owner == PLAYER {
            self.cues.push(Cue::HitConfirm);
        }
        if struck == Struck::Destroyed {
            self.destroyed(i, sh.owner);
        }
    }

    fn destroyed(&mut self, i: usize, by: u32) {
        let s = self.ships[i].clone();
        log::info!("destroyed {} ({}) by {by}", s.callsign, s.class.name);
        let big = s.class.size != Size::Light;
        self.blasts.push(Blast { pos: s.pos, vel: s.vel * 0.3, radius: s.class.radius * if big { 2.6 } else { 1.8 }, age: 0.0, life: if big { 1.1 } else { 0.7 } });
        let n = if big { 50 } else { 28 };
        self.spray(s.pos, s.vel * 0.3, n, SparkKind::Debris);
        if s.id == PLAYER {
            self.cues.push(Cue::PlayerLost);
            self.phase = Phase::Lost;
            self.phase_t = 0.0;
            self.target = None;
            return;
        }
        self.cues.push(if big { Cue::BigKill } else { Cue::Kill });
        if by == PLAYER {
            self.kills += 1;
            self.score += s.class.bounty;
            let line = KILLED[self.rng.below(KILLED.len() as u32) as usize].replace("{}", s.class.name);
            self.say_traffic(line);
        }
    }

    fn spray(&mut self, pos: V2, vel: V2, n: usize, kind: SparkKind) {
        let speed = match kind {
            SparkKind::Debris => 220.0,
            SparkKind::Rock => 90.0,
            _ => 140.0,
        };
        for _ in 0..n {
            let dir = V2::from_heading(self.rng.range(-3.15, 3.15));
            let v = vel + dir * self.rng.range(0.2, 1.0) * speed;
            let life = self.rng.range(0.25, if kind == SparkKind::Debris { 1.4 } else { 0.5 });
            self.sparks.push(Spark { pos, vel: v, age: 0.0, life, kind });
        }
    }

    /// Ships against rocks and the station: pushed out, bounced, scraped.
    fn collide(&mut self, _dt: f32) {
        let mut bodies: Vec<(V2, f32)> = self.rocks.iter().map(|r| (r.pos, r.radius * 0.92)).collect();
        bodies.push((self.station.pos, self.station.radius * 0.95));
        let mut scrapes = Vec::new();
        for (i, s) in self.ships.iter_mut().enumerate() {
            if !s.alive {
                continue;
            }
            for &(c, r) in &bodies {
                let d = s.pos - c;
                let min = r + s.class.radius;
                let l = d.len();
                if l < min && l > 1e-3 {
                    let n = d * (1.0 / l);
                    s.pos = c + n * min;
                    let into = s.vel.dot(n);
                    if into < 0.0 {
                        s.vel -= n * (into * 1.6);
                        if -into > 60.0 {
                            scrapes.push((i, -into, n));
                        }
                    }
                }
            }
        }
        for (i, impact, n) in scrapes {
            let dmg = (impact - 60.0) * 0.08;
            let pos = self.ships[i].pos - n * self.ships[i].class.radius;
            self.spray(pos, V2::ZERO, 6, SparkKind::Rock);
            let id = self.ships[i].id;
            let struck = self.ships[i].take_hit(dmg, (-n).heading());
            if id == PLAYER {
                self.cues.push(Cue::ArmourHit);
            }
            if struck == Struck::Destroyed {
                self.destroyed(i, 0);
            }
        }
    }

    fn age_effects(&mut self, dt: f32) {
        for s in &mut self.sparks {
            s.pos += s.vel * dt;
            s.vel = s.vel * (1.0 - 1.5 * dt);
            s.age += dt;
        }
        self.sparks.retain(|s| s.age < s.life);
        for b in &mut self.blasts {
            b.pos += b.vel * dt;
            b.age += dt;
        }
        self.blasts.retain(|b| b.age < b.life);
    }

    /// The dead are cleared; pirates that ran far enough leave the region.
    fn tidy(&mut self) {
        let player_pos = self.ships.iter().find(|s| s.id == PLAYER && s.alive).map(|p| p.pos);
        let mut left = Vec::new();
        for s in &self.ships {
            if s.alive && s.side == Side::Pirate && player_pos.is_some_and(|p| (s.pos - p).len() > LEAVE_RANGE) {
                left.push(s.id);
            }
        }
        for id in &left {
            if let Some(s) = self.ships.iter_mut().find(|s| s.id == *id) {
                s.alive = false;
                log::info!("{} left the region", s.callsign);
            }
        }
        // the player's wreck stays in the list (dead) until the relaunch
        self.ships.retain(|s| s.alive || s.id == PLAYER);
        let ids: Vec<u32> = self.ships.iter().map(|s| s.id).collect();
        self.brains.retain(|id, _| ids.contains(id));
        // a pirate turning tail says so, once
        let mut running = Vec::new();
        for (id, b) in &mut self.brains {
            if b.mode == ai::Mode::Flee && !b.announced {
                b.announced = true;
                running.push(*id);
            }
        }
        for id in running {
            if let Some(s) = self.ship(id) {
                let from = s.callsign.clone();
                let line = RUNNING[self.rng.below(RUNNING.len() as u32) as usize];
                self.say(from, line, Some(Side::Pirate));
            }
        }
    }

    fn waves(&mut self, dt: f32) {
        if self.hostiles().next().is_some() || self.phase != Phase::Flying {
            return;
        }
        self.lull -= dt;
        if self.lull > 0.0 {
            return;
        }
        self.lull = LULL;
        self.wave += 1;
        self.spawn_wave();
    }

    /// The make-up of wave `n`: raiders always, heavy fighters from the
    /// second, a gunboat every third from the fourth.
    pub fn wave_classes(n: u32) -> Vec<&'static Class> {
        let mut v: Vec<&'static Class> = Vec::new();
        if n >= 4 && n % 3 == 1 {
            v.push(&ship::KARHU);
        }
        let heavies = n.saturating_sub(1).div_ceil(2).min(3);
        for _ in 0..heavies {
            v.push(&ship::AHMA);
        }
        let raiders = 1 + n.div_ceil(2).min(3);
        for _ in 0..raiders {
            v.push(&ship::KARPPA);
        }
        v.truncate(7);
        v
    }

    fn spawn_wave(&mut self) {
        let Some(me) = self.player() else { return };
        let centre = me.pos;
        let bearing = self.rng.range(-3.14, 3.14);
        let at = centre + V2::from_heading(bearing) * self.rng.range(3200.0, 3800.0);
        let classes = World::wave_classes(self.wave);
        log::info!("wave {} arrives: {:?}", self.wave, classes.iter().map(|c| c.name).collect::<Vec<_>>());
        for (k, class) in classes.iter().enumerate() {
            let id = self.next_id;
            self.next_id += 1;
            let offset = V2::from_heading(bearing + 1.57) * ((k as f32 - classes.len() as f32 / 2.0) * 180.0);
            let pos = at + offset;
            let heading = (centre - pos).heading();
            let callsign = format!("{} {}", class.name, callsign(&mut self.rng));
            let mut s = Ship::new(id, class, Side::Pirate, callsign, pos, heading);
            s.vel = V2::from_heading(heading) * class.max_speed;
            let base = match class.size {
                Size::Light => 0.3,
                Size::Medium => 0.45,
                Size::Heavy => 0.55,
            };
            let skill = (base + self.wave as f32 * 0.04 + self.rng.range(-0.1, 0.1)).min(0.9);
            let flee_at = match class.size {
                Size::Light => self.rng.range(0.3, 0.6),
                Size::Medium => self.rng.range(0.15, 0.35),
                Size::Heavy => 0.0,
            };
            self.brains.insert(id, Brain::new(skill, flee_at));
            self.ships.push(s);
        }
        self.cues.push(Cue::Contact);
        let from = self.ships.last().map(|s| s.callsign.clone()).unwrap_or_default();
        let line = ARRIVALS[self.rng.below(ARRIVALS.len() as u32) as usize];
        self.say(from, line, Some(Side::Pirate));
        self.repair_told = false;
    }

    /// Near the station with no pirates about, the dock crews patch you up.
    fn repair(&mut self, dt: f32) {
        if self.hostiles().next().is_some() {
            return;
        }
        let station = self.station.pos;
        let reach = self.station.radius + 900.0;
        let Some(p) = self.ships.iter_mut().find(|s| s.id == PLAYER && s.alive) else { return };
        if (p.pos - station).len() > reach {
            return;
        }
        let c = p.class;
        let mut mended = false;
        for (a, max) in p.armour.iter_mut().zip(c.armour) {
            if *a < max {
                *a = (*a + 3.0 * dt).min(max);
                mended = true;
            }
        }
        if p.hull < c.hull {
            p.hull = (p.hull + 2.0 * dt).min(c.hull);
            mended = true;
        }
        if mended && !self.repair_told {
            self.repair_told = true;
            self.say_traffic("Hold steady, dock crews are patching your plating.");
        }
    }

    /// One line of state for the session log.
    pub fn snapshot(&self) -> String {
        match self.player() {
            Some(p) => format!(
                "t={:.1} phase={:?} pos=({:.0},{:.0}) hdg={:.2} spd={:.0}/{:.0} burn={} energy={:.0} shields={:?} armour={:?} hull={:.0} target={:?} hostiles={} shots={} wave={} kills={}",
                self.time,
                self.phase,
                p.pos.x,
                p.pos.y,
                p.heading,
                p.speed(),
                p.set_speed,
                p.burning,
                p.energy,
                p.shields.map(|x| x.round() as i32),
                p.armour.map(|x| x.round() as i32),
                p.hull,
                self.target,
                self.hostiles().count(),
                self.shots.len(),
                self.wave,
                self.kills
            ),
            None => format!("t={:.1} phase={:?} hostiles={} wave={} kills={}", self.time, self.phase, self.hostiles().count(), self.wave, self.kills),
        }
    }
}

/// Whether the segment a..b passes within `r` of `c`.
fn seg_hits(a: V2, b: V2, c: V2, r: f32) -> bool {
    let ab = b - a;
    let l2 = ab.dot(ab);
    let t = if l2 > 0.0 { ((c - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    ((a + ab * t) - c).len() <= r
}

/// The region's rocks: a scatter round the station, none too close to it.
fn scatter_rocks(rng: &mut Rng) -> Vec<Rock> {
    let mut rocks: Vec<Rock> = Vec::new();
    let mut tries = 0;
    while rocks.len() < 28 && tries < 2000 {
        tries += 1;
        let r = if rng.chance(0.15) { rng.range(260.0, 420.0) } else { rng.range(50.0, 200.0) };
        let pos = V2::from_heading(rng.range(-3.14, 3.14)) * rng.range(1300.0, 9000.0);
        if rocks.iter().any(|o| (o.pos - pos).len() < o.radius + r + 250.0) {
            continue;
        }
        rocks.push(Rock { pos, radius: r, seed: rng.next_u64() as u32 });
    }
    rocks
}

/// Pirate callsigns: placeholder names until the Reach's own are written.
fn callsign(rng: &mut Rng) -> &'static str {
    const NAMES: [&str; 12] = ["Rust", "Vilu", "Hiisi", "Kide", "Nokka", "Routio", "Sisu", "Kalma", "Tuisku", "Rae", "Kivi", "Pakkanen"];
    NAMES[rng.below(NAMES.len() as u32) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_grow() {
        assert_eq!(World::wave_classes(1).len(), 2);
        assert!(World::wave_classes(4).iter().any(|c| c.size == Size::Heavy));
        assert!(World::wave_classes(30).len() <= 7);
    }

    #[test]
    fn a_wave_arrives_and_the_field_fights() {
        let mut w = World::new(7);
        let idle = Intent::default();
        for _ in 0..(60 * 6) {
            w.update(&idle, 1.0 / 60.0);
        }
        assert_eq!(w.wave, 1);
        assert!(w.hostiles().count() >= 2);
        assert!(w.target.is_some(), "a target is picked for you");
        // a minute of an idle player: the pirates find you and shoot
        for _ in 0..(60 * 60) {
            w.update(&idle, 1.0 / 60.0);
        }
        let p = &w.ships[0];
        let hurt = p.shields.iter().sum::<f32>() < ship::TIIRA.shield * 4.0 || p.hull < ship::TIIRA.hull || !p.alive;
        assert!(hurt, "an idle player gets shot at");
    }

    #[test]
    fn a_shot_hits_what_it_passes_through() {
        assert!(seg_hits(v2(0.0, 0.0), v2(100.0, 0.0), v2(50.0, 10.0), 20.0));
        assert!(!seg_hits(v2(0.0, 0.0), v2(100.0, 0.0), v2(150.0, 0.0), 20.0));
    }

    #[test]
    fn the_rocks_leave_the_dock_clear() {
        let w = World::new(1);
        assert!(w.rocks.len() > 10);
        assert!(w.rocks.iter().all(|r| r.pos.len() - r.radius > 1000.0));
    }
}
