//! The simulation: the Northern Reach's belt regions, the bases and their
//! markets, the player's ship and hold, the clock, and the claim-jumpers.
//! No terminal code here (Torched's split), so it runs headless in tests and
//! in `--demo`.

pub mod ai;
pub mod cue;
pub mod econ;
pub mod faction;
pub mod rng;
pub mod save;
pub mod sector;
pub mod ship;
pub mod v2;

use ai::{Brain, Obstacle, Role};
use cue::Cue;
use econ::Market;
use faction::{Faction, Standing};
use rng::Rng;
use sector::{BaseKind, Sector};
use ship::{Class, Intent, Ship, Side, Size, Struck};
use std::collections::{HashMap, HashSet};
use v2::{v2, V2};

pub const PLAYER: u32 = 1;
/// Pirates further than this from the player give up the chase and leave.
const LEAVE_RANGE: f32 = 6500.0;
/// Shots this close to the player are heard.
const EARSHOT: f32 = 1600.0;
/// Close enough to a base to ask for landing, beyond its radius.
pub const LANDING_RANGE: f32 = 700.0;
/// Slow enough to land, m/s.
pub const LANDING_SPEED: f32 = 150.0;
/// Close enough to a lane mouth to jump.
pub const MOUTH_RADIUS: f32 = 1200.0;
/// Hostiles within this range block the autopilot and landing.
const CONTACT_RANGE: f32 = 5000.0;
/// The autopilot's drive speed, m/s.
const DRIVE: f32 = 6000.0;
/// Game hours a lane crossing takes.
const LANE_HOURS: f64 = 4.0;
const START_CREDITS: u32 = 2000;

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: V2,
    pub vel: V2,
    pub owner: u32,
    pub side: Side,
    pub faction: Faction,
    pub damage: f32,
    /// metres left to fly
    pub left: f32,
}

/// A Braille spark: hit flashes, debris.
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
    pub seed: u32,
}

/// A base in the current region.
#[derive(Clone, Debug)]
pub struct Station {
    pub name: &'static str,
    pub kind: BaseKind,
    pub pos: V2,
    pub radius: f32,
    /// its market's index in `World::markets`
    pub market: usize,
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
    /// landed at base `n` of the current region
    Docked(usize),
    /// the player's ship is gone
    Lost,
}

/// A place in the current region to fly to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Base(usize),
    /// the lane mouth toward that region
    Mouth(usize),
}

pub struct World {
    pub seed: u64,
    pub rng: Rng,
    /// seconds of play, for animation
    pub time: f32,
    /// game hours since the start
    pub clock: f64,
    pub sector: Sector,
    pub region: usize,
    pub bases: Vec<Station>,
    pub rocks: Vec<Rock>,
    /// one per base in the sector, in `Sector::all_bases` order
    pub markets: Vec<Market>,
    pub credits: u32,
    pub cargo: [u32; econ::N],
    pub standing: Standing,
    /// ships the player has fired on, which now fight back
    pub angry: HashSet<u32>,
    /// what the player paid per unit, on average, for what is in the hold
    pub paid: [u32; econ::N],
    pub ships: Vec<Ship>,
    brains: HashMap<u32, Brain>,
    pub shots: Vec<Shot>,
    pub sparks: Vec<Spark>,
    pub blasts: Vec<Blast>,
    pub comms: Vec<Comm>,
    pub cues: Vec<Cue>,
    pub target: Option<u32>,
    pub nav: Option<Nav>,
    /// regions still to cross to reach a plotted destination
    pub course: Vec<usize>,
    pub autopilot: bool,
    pub encounters: u32,
    pub kills: u32,
    pub phase: Phase,
    /// seconds in the current phase
    pub phase_t: f32,
    /// seconds until the next encounter may come
    encounter_t: f32,
    /// demo and title: encounters come thick and fast
    pub busy: bool,
    next_id: u32,
}

const ARRIVALS: [&str; 6] = [
    "This rock's claimed. Turn round or be salvage.",
    "Nice little hull. We'll take it.",
    "Cargo out the hatch and you might live.",
    "Nobody's patrolling this far up.",
    "Fresh saalis, everyone. Go.",
    "Nobody flies the high orbits for free.",
];
const RUNNING: [&str; 4] = ["Not worth it. I'm out.", "Breaking off! Breaking off!", "You'll keep.", "Too hot. Going dark."];
const KILLED: [&str; 3] = ["Bounty logged: {}.", "Confirmed. One {} less.", "Good kill. {} down."];

impl World {
    /// A new game: docked at Routa Station in Pallas Reach.
    pub fn new(seed: u64) -> World {
        let sector = Sector::reach();
        let markets = sector.all_bases().iter().map(|&(r, b)| Market::new(sector.regions[r].bases[b].kind)).collect();
        let mut w = World {
            seed,
            rng: Rng::new(seed),
            time: 0.0,
            clock: 8.0,
            sector,
            region: 0,
            bases: Vec::new(),
            rocks: Vec::new(),
            markets,
            credits: START_CREDITS,
            cargo: [0; econ::N],
            standing: Standing::default(),
            angry: HashSet::new(),
            paid: [0; econ::N],
            ships: Vec::new(),
            brains: HashMap::new(),
            shots: Vec::new(),
            sparks: Vec::new(),
            blasts: Vec::new(),
            comms: Vec::new(),
            cues: Vec::new(),
            target: None,
            nav: None,
            course: Vec::new(),
            autopilot: false,
            encounters: 0,
            kills: 0,
            phase: Phase::Docked(0),
            phase_t: 0.0,
            encounter_t: 60.0,
            busy: false,
            next_id: PLAYER + 1,
        };
        w.enter_region(0);
        let mut s = Ship::new(PLAYER, &ship::TIIRA, Side::Player, "you", w.bases[0].pos, 0.0);
        s.vel = V2::ZERO;
        s.set_speed = 0.0;
        w.ships.push(s);
        w
    }

    /// The sandbox of the demo and the title: launched, with company coming.
    pub fn demo(seed: u64) -> World {
        let mut w = World::new(seed);
        w.busy = true;
        w.encounter_t = 4.0;
        w.launch();
        w
    }

    fn enter_region(&mut self, r: usize) {
        self.region = r;
        let all = self.sector.all_bases();
        self.bases = self.sector.regions[r]
            .bases
            .iter()
            .enumerate()
            .map(|(i, b)| Station { name: b.name, kind: b.kind, pos: b.pos, radius: b.radius, market: all.iter().position(|&x| x == (r, i)).unwrap_or(0) })
            .collect();
        self.rocks = sector::rocks(self.seed, r, &self.sector).into_iter().map(|k| Rock { pos: k.pos, radius: k.radius, seed: k.seed }).collect();
        self.ships.retain(|s| s.id == PLAYER);
        self.brains.clear();
        self.shots.clear();
        self.sparks.clear();
        self.blasts.clear();
        self.target = None;
        self.autopilot = false;
        let threat = self.sector.regions[r].threat;
        self.encounter_t = if threat == 0 { f32::INFINITY } else { self.rng.range(40.0, 110.0) / threat as f32 };
    }

    pub fn region_name(&self) -> &'static str {
        self.sector.regions[self.region].name
    }

    pub fn player(&self) -> Option<&Ship> {
        self.ships.iter().find(|s| s.id == PLAYER && s.alive)
    }

    fn player_mut(&mut self) -> Option<&mut Ship> {
        self.ships.iter_mut().find(|s| s.id == PLAYER && s.alive)
    }

    pub fn ship(&self, id: u32) -> Option<&Ship> {
        self.ships.iter().find(|s| s.id == id && s.alive)
    }

    /// Ships that will attack the player: pirates, ships the player has
    /// fired on, and patrols of factions that hate the player.
    pub fn hostiles(&self) -> impl Iterator<Item = &Ship> {
        self.ships.iter().filter(|s| self.hostile_to_player(s))
    }

    pub fn hostile_to_player(&self, s: &Ship) -> bool {
        s.alive
            && s.id != PLAYER
            && s.side != Side::Trader
            && (s.side == Side::Pirate || self.angry.contains(&s.id) || self.standing.of(s.faction) <= faction::HOSTILE)
    }

    /// The owner of the current region.
    pub fn owner(&self) -> Faction {
        self.sector.regions[self.region].owner
    }

    /// Hostiles within `range` of the player.
    fn hostiles_near(&self, range: f32) -> bool {
        let Some(p) = self.player() else { return false };
        self.hostiles().any(|s| (s.pos - p.pos).len() < range)
    }

    pub fn fleeing(&self, id: u32) -> bool {
        self.brains.get(&id).is_some_and(|b| b.mode == ai::Mode::Flee)
    }

    pub fn obstacles(&self) -> Vec<Obstacle> {
        let mut o: Vec<Obstacle> = self.rocks.iter().map(|r| Obstacle { pos: r.pos, radius: r.radius }).collect();
        o.extend(self.bases.iter().map(|b| Obstacle { pos: b.pos, radius: b.radius }));
        o
    }

    pub fn cargo_used(&self) -> u32 {
        self.cargo.iter().sum()
    }

    pub fn cargo_room(&self) -> u32 {
        self.ships.first().map_or(0, |s| s.class.cargo).saturating_sub(self.cargo_used())
    }

    /// Day and time, for the HUD.
    pub fn date(&self) -> String {
        let day = (self.clock / 24.0).floor() as u64 + 1;
        let mins = ((self.clock % 24.0) * 60.0) as u64;
        format!("day {day} {:02}:{:02}", mins / 60, mins % 60)
    }

    fn pass_hours(&mut self, h: f64) {
        let before = self.clock.floor();
        self.clock += h;
        let whole = self.clock.floor() - before;
        if whole >= 1.0 {
            for m in &mut self.markets {
                m.drift(whole as f32);
            }
        }
    }

    // --- comms ---------------------------------------------------------

    fn say(&mut self, from: impl Into<String>, text: impl Into<String>, side: Option<Side>) {
        let (from, text) = (from.into(), text.into());
        log::info!("comms {from}: {text}");
        self.comms.push(Comm { from, text, side, age: 0.0 });
        if self.comms.len() > 20 {
            self.comms.remove(0);
        }
        self.cues.push(Cue::Comms);
    }

    /// A line from the player's own ship systems.
    pub fn say_note(&mut self, from: &str, text: &str) {
        self.say(from.to_string(), text.to_string(), Some(Side::Player));
    }

    /// A message from the nearest base's traffic control, or the lane
    /// beacons where there is no base.
    fn say_traffic(&mut self, text: impl Into<String>) {
        let from = match self.nearest_base() {
            Some(b) => format!("{} Traffic", self.bases[b].name.split(' ').next().unwrap_or("Base")),
            None => "Lane beacon".to_string(),
        };
        self.say(from, text, None);
    }

    fn nearest_base(&self) -> Option<usize> {
        let p = self.ships.first()?.pos;
        (0..self.bases.len()).min_by(|&a, &b| (self.bases[a].pos - p).len().total_cmp(&(self.bases[b].pos - p).len()))
    }

    // --- navigation ----------------------------------------------------

    /// Every place to fly to in this region: bases, then lane mouths.
    pub fn navs(&self) -> Vec<Nav> {
        let mut v: Vec<Nav> = (0..self.bases.len()).map(Nav::Base).collect();
        v.extend(self.sector.neighbours(self.region).into_iter().map(Nav::Mouth));
        v
    }

    pub fn nav_pos(&self, n: Nav) -> V2 {
        match n {
            Nav::Base(i) => self.bases[i].pos,
            Nav::Mouth(to) => self.sector.mouth(self.region, to),
        }
    }

    pub fn nav_name(&self, n: Nav) -> String {
        match n {
            Nav::Base(i) => self.bases[i].name.to_string(),
            Nav::Mouth(to) => format!("lane to {}", self.sector.regions[to].name),
        }
    }

    /// `[` and `]`: the previous or next nav point.
    pub fn cycle_nav(&mut self, step: i32) {
        let all = self.navs();
        if all.is_empty() {
            return;
        }
        let i = self.nav.and_then(|n| all.iter().position(|&x| x == n)).map_or(0, |i| (i as i32 + step).rem_euclid(all.len() as i32) as usize);
        self.nav = Some(all[i]);
        self.cues.push(Cue::Target);
    }

    /// Plots a course to region `dest`; the nav point becomes the first lane.
    pub fn plot(&mut self, dest: usize) {
        self.course = self.sector.route(self.region, dest);
        self.nav = self.course.first().map(|&r| Nav::Mouth(r));
        log::info!("course to {} via {:?}", self.sector.regions[dest].name, self.course);
    }

    /// G: the autopilot on or off. It won't engage with hostiles about.
    pub fn toggle_autopilot(&mut self) {
        if self.autopilot {
            self.autopilot = false;
            return;
        }
        if self.phase != Phase::Flying || self.nav.is_none() {
            self.say("Autopilot", "No nav point. [ ] picks one, N opens the map.", Some(Side::Player));
            return;
        }
        if self.hostiles_near(CONTACT_RANGE) {
            self.say("Autopilot", "Contacts close by. Not engaging.", Some(Side::Player));
            return;
        }
        self.autopilot = true;
        log::info!("autopilot on to {:?}", self.nav);
    }

    /// J: through the lane whose mouth the player is at.
    pub fn jump(&mut self) {
        if self.phase != Phase::Flying {
            return;
        }
        let Some(p) = self.player() else { return };
        let pos = p.pos;
        let at = self.sector.neighbours(self.region).into_iter().find(|&to| (self.sector.mouth(self.region, to) - pos).len() < MOUTH_RADIUS);
        let Some(to) = at else {
            self.say("Drive", "No lane mouth in range. Fly into one first.", Some(Side::Player));
            return;
        };
        let from = self.region;
        log::info!("jump {} -> {}", self.sector.regions[from].name, self.sector.regions[to].name);
        self.enter_region(to);
        self.pass_hours(LANE_HOURS);
        let mouth = self.sector.mouth(to, from) * 0.92;
        let heading = (-mouth).heading();
        if let Some(p) = self.player_mut() {
            p.pos = mouth;
            p.heading = heading;
            p.vel = V2::from_heading(heading) * 150.0;
            p.set_speed = 150.0;
        }
        if self.course.first() == Some(&to) {
            self.course.remove(0);
        }
        self.nav = match self.course.first() {
            Some(&next) => Some(Nav::Mouth(next)),
            None if !self.bases.is_empty() => Some(Nav::Base(0)),
            None => None,
        };
        self.cues.push(Cue::Burner);
        let def = &self.sector.regions[to];
        let (note, threat) = (format!("{}: {}.", def.name, def.note), def.threat);
        self.say("Lane beacon", note, None);
        if threat > 0 && self.rng.chance(0.2 * threat as f32) {
            self.encounter_t = self.rng.range(3.0, 8.0);
        }
    }

    /// L: land at the nearest base, if close and slow enough.
    pub fn land(&mut self) {
        if self.phase != Phase::Flying {
            return;
        }
        let Some(b) = self.nearest_base() else {
            self.say("Lane beacon", "No base in this region.", None);
            return;
        };
        let Some(p) = self.player() else { return };
        let d = (p.pos - self.bases[b].pos).len() - self.bases[b].radius;
        let speed = p.speed();
        if d > LANDING_RANGE {
            self.say_traffic(format!("Come within {} m to land.", LANDING_RANGE as u32));
        } else if speed > LANDING_SPEED {
            self.say_traffic(format!("Too fast. Under {} to land.", LANDING_SPEED as u32));
        } else if self.standing.of(self.owner()) <= faction::BANNED {
            self.say_traffic("You're not welcome here. Clear the approach.");
        } else if self.hostiles_near(3000.0) {
            self.say_traffic("Not with company on your tail.");
        } else {
            self.dock(b);
        }
    }

    fn dock(&mut self, b: usize) {
        self.phase = Phase::Docked(b);
        self.phase_t = 0.0;
        self.autopilot = false;
        self.target = None;
        let pos = self.bases[b].pos;
        if let Some(p) = self.player_mut() {
            p.pos = pos;
            p.vel = V2::ZERO;
            p.set_speed = 0.0;
            p.shields = [p.class.shield; 4];
            p.energy = p.class.energy;
            p.fuel = p.class.fuel;
        }
        self.ships.retain(|s| s.id == PLAYER);
        self.brains.clear();
        self.shots.clear();
        if self.nav == Some(Nav::Base(b)) && self.course.is_empty() {
            self.nav = None;
        }
        self.pass_hours(0.5);
        log::info!("docked at {}", self.bases[b].name);
    }

    /// Leaves the dock, out along the docking arm.
    pub fn launch(&mut self) {
        let Phase::Docked(b) = self.phase else { return };
        let st = &self.bases[b];
        let pos = st.pos + v2(0.0, -(st.radius + 260.0));
        if let Some(p) = self.player_mut() {
            p.pos = pos;
            p.heading = 0.0;
            p.set_speed = 120.0;
            p.vel = V2::from_heading(0.0) * 120.0;
        }
        self.phase = Phase::Flying;
        self.phase_t = 0.0;
        self.pass_hours(0.25);
        self.say_traffic("Clear of the dock. Fly safe.");
        log::info!("launched from {}", self.bases[b].name);
    }

    // --- trade and the dock ------------------------------------------

    pub fn docked_market(&self) -> Option<usize> {
        match self.phase {
            Phase::Docked(b) => Some(self.bases[b].market),
            _ => None,
        }
    }

    /// Buys up to `n` of good `g`; returns how many.
    pub fn buy(&mut self, g: usize, n: u32) -> u32 {
        let Some(m) = self.docked_market() else { return 0 };
        let mut got = 0;
        for _ in 0..n {
            let price = self.markets[m].buy_price(g);
            if !self.markets[m].has(g) || self.credits < price || self.cargo_room() == 0 {
                break;
            }
            self.credits -= price;
            let held = self.cargo[g];
            self.paid[g] = ((self.paid[g] * held + price) as f32 / (held + 1) as f32).round() as u32;
            self.cargo[g] += 1;
            self.markets[m].bought(g, 1);
            got += 1;
        }
        if got > 0 {
            log::info!("bought {got} {} credits={}", econ::GOODS[g].name, self.credits);
        }
        got
    }

    /// Sells up to `n` of good `g`; returns how many.
    pub fn sell(&mut self, g: usize, n: u32) -> u32 {
        let Some(m) = self.docked_market() else { return 0 };
        let k = n.min(self.cargo[g]);
        for _ in 0..k {
            self.credits += self.markets[m].sell_price(g);
            self.markets[m].sold(g, 1);
            self.cargo[g] -= 1;
        }
        if self.cargo[g] == 0 {
            self.paid[g] = 0;
        }
        if k > 0 {
            log::info!("sold {k} {} credits={}", econ::GOODS[g].name, self.credits);
        }
        k
    }

    /// What repairing armour and hull costs here.
    pub fn repair_cost(&self) -> u32 {
        let Some(p) = self.ships.first() else { return 0 };
        let armour: f32 = p.class.armour.iter().zip(p.armour).map(|(m, a)| m - a).sum();
        (armour * 12.0 + (p.class.hull - p.hull) * 20.0).round() as u32
    }

    /// Repairs as much as the credits allow; returns whether anything was.
    pub fn repair(&mut self) -> bool {
        let cost = self.repair_cost();
        if cost == 0 || self.docked_market().is_none() {
            return false;
        }
        let k = (self.credits as f32 / cost as f32).min(1.0);
        let paid = (cost as f32 * k).round() as u32;
        self.credits -= paid.min(self.credits);
        if let Some(p) = self.player_mut() {
            for (a, m) in p.armour.iter_mut().zip(p.class.armour) {
                *a += (m - *a) * k;
            }
            p.hull += (p.class.hull - p.hull) * k;
        }
        self.pass_hours(1.0);
        log::info!("repaired {:.0}% for {paid}", k * 100.0);
        true
    }

    // --- comms ---------------------------------------------------------

    /// Who C hails: the target, or the nearest ship within 6 km.
    pub fn hail_target(&self) -> Option<u32> {
        let me = self.player()?;
        self.target.filter(|t| self.ship(*t).is_some()).or_else(|| {
            self.ships
                .iter()
                .filter(|s| s.alive && s.id != PLAYER && (s.pos - me.pos).len() < 6000.0)
                .min_by(|a, b| (a.pos - me.pos).len().total_cmp(&(b.pos - me.pos).len()))
                .map(|s| s.id)
        })
    }

    /// The pirates the player is up against: those within 6 km.
    fn pack(&self) -> Vec<u32> {
        let Some(me) = self.player() else { return Vec::new() };
        self.ships.iter().filter(|s| s.alive && s.side == Side::Pirate && (s.pos - me.pos).len() < 6000.0).map(|s| s.id).collect()
    }

    fn bribe(&self) -> u32 {
        120 * self.pack().len().max(1) as u32
    }

    /// What the player can say to ship `id`.
    pub fn hail_lines(&self, id: u32) -> Vec<String> {
        let Some(s) = self.ship(id) else { return Vec::new() };
        match s.side {
            Side::Pirate => vec![
                "Leave me be. Nothing in my hold is worth the fight.".into(),
                format!("Take {} cr and go.", self.bribe()),
                "Come and try it.".into(),
            ],
            Side::Patrol if self.hostile_to_player(s) => vec!["Hold your fire! It was a mistake.".into()],
            Side::Patrol => vec!["Anything moving out here?".into(), "Requesting an escort.".into()],
            Side::Trader => vec!["Where's the money this week?".into(), "Fly safe.".into()],
            Side::Player => Vec::new(),
        }
    }

    /// Says line `choice` to ship `id`; the answer comes over comms.
    pub fn hail(&mut self, id: u32, choice: usize) {
        let Some(s) = self.ship(id).cloned() else { return };
        log::info!("hail {} choice {choice}", s.callsign);
        let from = s.callsign.clone();
        let voice = Some(s.side);
        match (s.side, choice) {
            (Side::Pirate, 0) => {
                let empty = self.cargo_used() < 5;
                let chance = if empty { 0.55 } else { 0.2 };
                if self.rng.chance(chance) {
                    self.say(from, "Empty hulls aren't worth the fuel. Go on, then.", voice);
                    self.call_off();
                } else {
                    self.say(from, "We'll see what's in it ourselves.", voice);
                }
            }
            (Side::Pirate, 1) => {
                let price = self.bribe();
                if self.credits >= price {
                    self.credits -= price;
                    self.standing.shift(Faction::Pirates, 3);
                    self.say(from, "Pleasure doing business. Off you go.", voice);
                    self.call_off();
                } else {
                    self.say(from, "You haven't got it. Pity.", voice);
                }
            }
            (Side::Pirate, _) => {
                for id in self.pack() {
                    if let Some(b) = self.brains.get_mut(&id) {
                        b.flee_at = 0.0;
                        b.mode = ai::Mode::Attack;
                    }
                }
                self.say(from, "Ha! Your funeral.", voice);
            }
            (Side::Patrol, 0) if self.hostile_to_player(&s) => {
                let f = s.faction;
                if self.standing.of(f) > faction::HOSTILE && self.rng.chance(0.5) {
                    self.angry.remove(&id);
                    self.say(from, "Fine. Watch where you point those guns.", voice);
                } else {
                    self.say(from, "Too late for that.", voice);
                }
            }
            (Side::Patrol, 0) => {
                let line = self.rumour();
                self.say(from, line, voice);
            }
            (Side::Patrol, _) => {
                if self.standing.of(s.faction) >= 25 {
                    if let Some(b) = self.brains.get_mut(&id) {
                        b.role = Role::Escort;
                    }
                    self.say(from, "On your wing until you leave the region.", voice);
                } else {
                    self.say(from, "We don't know you well enough. Earn it.", voice);
                }
            }
            (Side::Trader, 0) => {
                let tip = self.tip();
                self.say(from, tip, voice);
            }
            (Side::Trader, _) => {
                self.standing.shift(Faction::Traders, 1);
                self.say(from, "And you. Watch the Gap.", voice);
            }
            _ => {}
        }
    }

    /// Every pirate nearby breaks off and runs.
    fn call_off(&mut self) {
        for id in self.pack() {
            if let Some(b) = self.brains.get_mut(&id) {
                b.mode = ai::Mode::Flee;
                b.flee_at = 1.0;
                b.announced = true;
            }
        }
    }

    /// A patrol's news: how quiet this region is, and a word on the Reach.
    fn rumour(&mut self) -> String {
        let here = match self.sector.regions[self.region].threat {
            0 => "Quiet as a tomb here.",
            1 => "A few claim-jumpers, nothing we can't handle.",
            2 => "Busy. Keep your shields up.",
            _ => "You're on your own out here. We don't come this far.",
        };
        const MORE: [&str; 4] = [
            "Kalmisto's crawling with them this week.",
            "Customs at the Gate are checking holds again.",
            "Halla's paying over the odds for ore.",
            "Revontuli Gap's no place to fly alone.",
        ];
        format!("{here} {}", MORE[self.rng.below(MORE.len() as u32) as usize])
    }

    /// A trader's tip: where one good sells best right now.
    fn tip(&mut self) -> String {
        let g = self.rng.below(econ::N as u32) as usize;
        let all = self.sector.all_bases();
        let best = (0..self.markets.len()).max_by_key(|&m| self.markets[m].sell_price(g));
        match best {
            Some(m) => {
                let (r, b) = all[m];
                let base = self.sector.regions[r].bases[b].name;
                format!("{} fetches {} at {base}.", econ::GOODS[g].name, self.markets[m].sell_price(g))
            }
            None => "Nothing worth the fuel.".into(),
        }
    }

    // --- the player's own targeting ----------------------------------

    /// T: every ship in turn, nearest first.
    pub fn cycle_target(&mut self) {
        let Some(me) = self.player() else { return };
        let mut ids: Vec<(f32, u32)> = self.ships.iter().filter(|s| s.alive && s.id != PLAYER).map(|s| ((s.pos - me.pos).len(), s.id)).collect();
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
        let Some(me) = self.player() else { return };
        let next = self.hostiles().min_by(|a, b| (a.pos - me.pos).len().total_cmp(&(b.pos - me.pos).len())).map(|s| s.id);
        if next.is_some() && next != self.target {
            self.cues.push(Cue::Target);
        }
        self.target = next;
    }

    pub fn match_speed(&self) -> Option<f32> {
        self.target.and_then(|t| self.ship(t)).map(|t| t.speed())
    }

    // --- the tick ------------------------------------------------------

    /// One tick: `player` is what the person (or the demo autopilot) asks.
    pub fn update(&mut self, player: &Intent, dt: f32) {
        self.time += dt;
        self.phase_t += dt;
        for c in &mut self.comms {
            c.age += dt;
        }
        if let Phase::Docked(_) = self.phase {
            return;
        }
        // a game minute a second while flying
        self.pass_hours(dt as f64 / 60.0);
        if self.autopilot {
            self.drive(dt);
        }
        let obstacles = self.obstacles();
        let player_pos = self.player().map(|p| p.pos);

        let mut intents = Vec::with_capacity(self.ships.len());
        for s in &self.ships {
            let it = if s.id == PLAYER {
                if self.autopilot { Intent { set_speed: Some(s.set_speed), ..Default::default() } } else { *player }
            } else if let Some(b) = self.brains.get_mut(&s.id) {
                // the nearest enemy in reach; a trader only looks for danger;
                // an escort fights what comes near the player
                let reach = match b.role {
                    Role::Trader => 2500.0,
                    _ => 6000.0,
                };
                let centre = if b.role == Role::Escort { player_pos.unwrap_or(s.pos) } else { s.pos };
                b.target = self
                    .ships
                    .iter()
                    .filter(|o| (o.pos - centre).len() < reach && fights_of(&self.standing, &self.angry, o, s))
                    .min_by(|x, y| (x.pos - s.pos).len().total_cmp(&(y.pos - s.pos).len()))
                    .map(|o| o.id);
                if b.role == Role::Escort {
                    b.goal = self.ships.first().filter(|p| p.alive).map(|p| p.pos - p.forward() * 250.0 + p.forward().right() * 120.0);
                }
                ai::think(b, s, &self.ships, &obstacles, &mut self.rng, dt)
            } else {
                Intent::default()
            };
            intents.push(it);
        }
        for (i, it) in intents.iter().enumerate() {
            let near = player_pos.is_some_and(|p| (self.ships[i].pos - p).len() < EARSHOT);
            let s = &mut self.ships[i];
            if !s.alive || (s.id == PLAYER && self.autopilot) {
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
                        faction: s.faction,
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
        self.collide();
        self.age_effects(dt);
        self.tidy();
        self.encounter(dt);
        if self.target.is_some_and(|t| self.ship(t).is_none()) {
            self.target = None;
        }
        if self.target.is_none() && self.hostiles().next().is_some() && self.player().is_some() {
            self.nearest_target();
        }
    }

    /// The autopilot: straight to the nav point at drive speed, dropping out
    /// on arrival or when something turns up on the way.
    fn drive(&mut self, dt: f32) {
        let Some(nav) = self.nav else {
            self.autopilot = false;
            return;
        };
        let goal = self.nav_pos(nav);
        let stop = match nav {
            Nav::Base(i) => self.bases[i].radius + LANDING_RANGE * 0.6,
            Nav::Mouth(_) => MOUTH_RADIUS * 0.3,
        };
        if self.hostiles_near(CONTACT_RANGE) {
            self.autopilot = false;
            self.say("Autopilot", "Contact! Dropping out.", Some(Side::Player));
            self.cues.push(Cue::Contact);
            return;
        }
        let threat = self.sector.regions[self.region].threat as f32;
        let Some(p) = self.player() else { return };
        let to = goal - p.pos;
        let left = to.len() - stop;
        if left <= 0.0 {
            self.autopilot = false;
            let name = self.nav_name(nav);
            let (hint, speed) = match nav {
                Nav::Mouth(_) => (" J to jump.", 30.0),
                Nav::Base(_) => (" L to land.", 60.0),
            };
            self.say("Autopilot", format!("Arrived: {name}.{hint}"), Some(Side::Player));
            if let Some(p) = self.player_mut() {
                p.set_speed = speed;
                p.vel = p.forward() * speed;
            }
            return;
        }
        let step = (DRIVE * dt).min(left);
        // claim-jumpers lie in wait along the way: a few chances a kilometre
        if !self.busy && threat > 0.0 && self.rng.chance(step / 1000.0 * 0.035 * threat) {
            self.autopilot = false;
            self.spawn_encounter();
            return;
        }
        let dir = to.norm();
        if let Some(p) = self.player_mut() {
            p.heading = dir.heading();
            p.pos += dir * step;
            p.vel = dir * p.class.max_speed;
        }
        // driving takes the time flying it would have, at cruise
        self.pass_hours((step / 300.0 / 3600.0 * 60.0) as f64);
    }

    fn move_shots(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|sh| {
            let from = sh.pos;
            let step = sh.vel * dt;
            sh.pos += step;
            sh.left -= step.len();
            // anything but the shooter and its own faction's other ships
            let owner_npc = sh.owner != PLAYER;
            let hit = self.ships.iter().position(|s| {
                s.alive && s.id != sh.owner && !(owner_npc && s.id != PLAYER && s.faction == sh.faction) && seg_hits(from, sh.pos, s.pos, s.class.radius)
            });
            if let Some(i) = hit {
                self.hit(i, sh);
                return false;
            }
            let solid = self.rocks.iter().any(|r| (sh.pos - r.pos).len() < r.radius * 0.9) || self.bases.iter().any(|b| (sh.pos - b.pos).len() < b.radius * 0.9);
            if solid {
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
            self.autopilot = false;
            self.cues.push(if struck == Struck::Shield { Cue::ShieldHit } else { Cue::ArmourHit });
        } else if sh.owner == PLAYER {
            self.cues.push(Cue::HitConfirm);
            let s = &self.ships[i];
            if s.side != Side::Pirate {
                let (f, callsign, first) = (s.faction, s.callsign.clone(), self.angry.insert(id));
                self.standing.shift(f, -3);
                if first {
                    log::info!("player fired on {callsign} ({:?})", f);
                    self.say(callsign, "You'll answer for that!", Some(s_side(f)));
                }
            }
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
            self.autopilot = false;
            return;
        }
        self.cues.push(if big { Cue::BigKill } else { Cue::Kill });
        if by == PLAYER && s.side != Side::Pirate {
            self.standing.shift(s.faction, -25);
            log::info!("player destroyed a {:?} ship", s.faction);
            return;
        }
        if by == PLAYER {
            self.kills += 1;
            self.credits += s.class.bounty;
            self.standing.shift(Faction::Pirates, -3);
            self.standing.shift(Faction::Traders, 1);
            let owner = self.owner();
            if owner != Faction::Pirates {
                self.standing.shift(owner, 2);
            }
            let line = KILLED[self.rng.below(KILLED.len() as u32) as usize].replace("{}", s.class.name);
            self.say_traffic(format!("{line} {} cr paid.", s.class.bounty));
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

    /// Ships against rocks and bases: pushed out, bounced, scraped.
    fn collide(&mut self) {
        let mut bodies: Vec<(V2, f32)> = self.rocks.iter().map(|r| (r.pos, r.radius * 0.92)).collect();
        bodies.extend(self.bases.iter().map(|b| (b.pos, b.radius * 0.95)));
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
        let player_pos = self.player().map(|p| p.pos);
        let arrived: HashSet<u32> = self.brains.iter().filter(|(_, b)| b.arrived).map(|(id, _)| *id).collect();
        for s in &mut self.ships {
            if s.id == PLAYER || !s.alive {
                continue;
            }
            let far = player_pos.is_some_and(|p| (s.pos - p).len() > if s.side == Side::Pirate { LEAVE_RANGE } else { LEAVE_RANGE * 2.0 });
            if far || arrived.contains(&s.id) {
                s.alive = false;
                log::info!("{} left the region", s.callsign);
            }
        }
        self.angry.retain(|id| self.ships.iter().any(|s| s.id == *id && s.alive));
        self.ships.retain(|s| s.alive || s.id == PLAYER);
        let ids: Vec<u32> = self.ships.iter().map(|s| s.id).collect();
        self.brains.retain(|id, _| ids.contains(id));
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

    /// Claim-jumpers turn up now and then, more often where the threat is
    /// higher; never in the quiet regions.
    /// Now and then someone turns up: claim-jumpers (more often where the
    /// threat is higher, never in quiet regions), the owner's patrols, or a
    /// freighter on its run.
    fn encounter(&mut self, dt: f32) {
        if self.phase != Phase::Flying || self.hostiles().next().is_some() {
            return;
        }
        self.encounter_t -= dt;
        if self.encounter_t > 0.0 {
            return;
        }
        let threat = self.sector.regions[self.region].threat as f32;
        self.encounter_t = if self.busy { 8.0 } else { self.rng.range(35.0, 80.0) };
        if self.busy {
            self.spawn_encounter();
            return;
        }
        let owner = self.owner();
        let patrol = if owner == Faction::Pirates { 0.0 } else { 1.2 };
        let trade = if self.bases.is_empty() { 0.4 } else { 1.4 };
        let roll = self.rng.range(0.0, threat + patrol + trade);
        if roll < threat {
            self.spawn_encounter();
        } else if roll < threat + patrol {
            self.spawn_traveller(Side::Patrol, owner);
        } else {
            self.spawn_traveller(Side::Trader, Faction::Traders);
        }
    }

    /// A patrol pair or a freighter: in at one nav point, out at another.
    fn spawn_traveller(&mut self, side: Side, faction: Faction) {
        let navs = self.navs();
        if navs.len() < 2 {
            return;
        }
        let a = self.rng.below(navs.len() as u32) as usize;
        let mut b = self.rng.below(navs.len() as u32 - 1) as usize;
        if b >= a {
            b += 1;
        }
        let (from, to) = (self.nav_pos(navs[a]), self.nav_pos(navs[b]));
        let start = match navs[a] {
            Nav::Base(i) => from + v2(0.0, -(self.bases[i].radius + 300.0)),
            Nav::Mouth(_) => from,
        };
        let heading = (to - start).heading();
        let crew: Vec<&'static Class> = match side {
            Side::Trader if self.rng.chance(0.4) => vec![&ship::HAAHKA, &ship::KOTKA],
            Side::Trader => vec![&ship::HAAHKA],
            _ => vec![&ship::KOTKA, &ship::KOTKA],
        };
        for (k, class) in crew.iter().enumerate() {
            let id = self.next_id;
            self.next_id += 1;
            let pos = start + V2::from_heading(heading + 1.57) * (k as f32 * 160.0);
            let name = callsign(&mut self.rng);
            let (prefix, role) = match (side, std::ptr::eq(*class, &ship::KOTKA)) {
                (Side::Trader, false) => ("Freighter", Role::Trader),
                (Side::Trader, true) => ("Escort", Role::Fighter),
                _ => (faction_prefix(faction), Role::Fighter),
            };
            let ship_side = if std::ptr::eq(*class, &ship::HAAHKA) { Side::Trader } else { Side::Patrol };
            let mut s = Ship::new(id, class, ship_side, format!("{prefix} {name}"), pos, heading);
            s.faction = faction;
            s.vel = V2::from_heading(heading) * class.max_speed * 0.6;
            let goal = to + (pos - start);
            self.brains.insert(id, Brain::new(0.6, if role == Role::Trader { 1.0 } else { 0.25 }).with_role(role, Some(goal)));
            self.ships.push(s);
        }
        log::info!("{:?} {:?} passing through {}", side, faction, self.region_name());
    }

    /// The make-up of the `n`th encounter in a region of `threat`.
    pub fn group(n: u32, threat: u32) -> Vec<&'static Class> {
        let level = n.min(8) + threat * 2;
        let mut v: Vec<&'static Class> = Vec::new();
        if level >= 7 && n % 3 == 1 {
            v.push(&ship::KARHU);
        }
        for _ in 0..level.saturating_sub(2).div_ceil(3).min(3) {
            v.push(&ship::AHMA);
        }
        for _ in 0..(1 + level.div_ceil(3)).min(4) {
            v.push(&ship::KARPPA);
        }
        v.truncate(6);
        v
    }

    fn spawn_encounter(&mut self) {
        let Some(me) = self.player() else { return };
        let centre = me.pos;
        self.encounters += 1;
        let bearing = self.rng.range(-3.14, 3.14);
        let at = centre + V2::from_heading(bearing) * self.rng.range(3200.0, 3800.0);
        let threat = self.sector.regions[self.region].threat;
        let classes = World::group(self.encounters, threat);
        log::info!("encounter {} in {}: {:?}", self.encounters, self.region_name(), classes.iter().map(|c| c.name).collect::<Vec<_>>());
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
            let skill = (base + threat as f32 * 0.08 + self.encounters.min(10) as f32 * 0.02 + self.rng.range(-0.1, 0.1)).min(0.9);
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
    }

    /// One line of state for the session log.
    pub fn snapshot(&self) -> String {
        let common = format!(
            "clock={:.2} region={} phase={:?} credits={} cargo={:?} nav={:?} course={:?} auto={} hostiles={} encounters={} kills={}",
            self.clock,
            self.region_name(),
            self.phase,
            self.credits,
            self.cargo,
            self.nav,
            self.course,
            self.autopilot,
            self.hostiles().count(),
            self.encounters,
            self.kills
        );
        match self.player() {
            Some(p) => format!(
                "{common} pos=({:.0},{:.0}) hdg={:.2} spd={:.0}/{:.0} burn={} energy={:.0} shields={:?} armour={:?} hull={:.0} target={:?} shots={}",
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
                self.shots.len()
            ),
            None => common,
        }
    }
}

/// `World::fights` without borrowing the world: whether `a` would fight `b`.
fn fights_of(standing: &Standing, angry: &HashSet<u32>, a: &Ship, b: &Ship) -> bool {
    let against_player = |s: &Ship| s.alive && s.id != PLAYER && s.side != Side::Trader && (s.side == Side::Pirate || angry.contains(&s.id) || standing.of(s.faction) <= faction::HOSTILE);
    if !a.alive || !b.alive || a.id == b.id {
        return false;
    }
    if a.id == PLAYER {
        return against_player(b);
    }
    if b.id == PLAYER {
        return against_player(a);
    }
    a.faction.at_war(b.faction)
}

fn faction_prefix(f: Faction) -> &'static str {
    match f {
        Faction::Coop => "Watch",
        Faction::Crown => "Crown",
        Faction::Traders => "Guild",
        Faction::Inner => "Customs",
        Faction::Pirates => "Raider",
    }
}

/// The comms colour for a faction's voice.
fn s_side(f: Faction) -> Side {
    if f == Faction::Pirates { Side::Pirate } else { Side::Patrol }
}

/// Whether the segment a..b passes within `r` of `c`.
fn seg_hits(a: V2, b: V2, c: V2, r: f32) -> bool {
    let ab = b - a;
    let l2 = ab.dot(ab);
    let t = if l2 > 0.0 { ((c - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    ((a + ab * t) - c).len() <= r
}

/// Pirate callsigns: placeholder names until the Reach's own are written.
fn callsign(rng: &mut Rng) -> &'static str {
    const NAMES: [&str; 12] = ["Rust", "Vilu", "Hiisi", "Kide", "Nokka", "Routio", "Sisu", "Kalma", "Tuisku", "Rae", "Kivi", "Pakkanen"];
    NAMES[rng.below(NAMES.len() as u32) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fly(w: &mut World, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            w.update(&Intent::default(), 1.0 / 60.0);
        }
    }

    #[test]
    fn a_new_game_starts_docked_at_routa() {
        let w = World::new(1);
        assert_eq!(w.phase, Phase::Docked(0));
        assert_eq!(w.bases[0].name, "Routa Station");
        assert_eq!(w.credits, START_CREDITS);
    }

    #[test]
    fn groups_grow_with_threat_and_stay_bounded() {
        assert!(World::group(1, 1).len() >= 2);
        assert!(World::group(1, 3).len() > World::group(1, 1).len());
        assert!(World::group(40, 3).len() <= 6);
    }

    #[test]
    fn the_demo_field_fights() {
        let mut w = World::demo(7);
        fly(&mut w, 6.0);
        assert!(w.hostiles().count() >= 2);
        assert!(w.target.is_some());
        fly(&mut w, 60.0);
        let p = &w.ships[0];
        let hurt = p.shields.iter().sum::<f32>() < ship::TIIRA.shield * 4.0 || p.hull < ship::TIIRA.hull || !p.alive;
        assert!(hurt, "an idle player gets shot at");
    }

    #[test]
    fn launch_fly_the_autopilot_and_land() {
        let mut w = World::new(3);
        w.launch();
        assert_eq!(w.phase, Phase::Flying);
        w.nav = Some(Nav::Base(0));
        // fly off a way first
        w.ships[0].pos = v2(5000.0, 0.0);
        w.toggle_autopilot();
        assert!(w.autopilot);
        w.encounter_t = f32::INFINITY;
        fly(&mut w, 3.0);
        assert!(!w.autopilot, "arrived");
        w.ships[0].vel = V2::ZERO;
        w.land();
        assert_eq!(w.phase, Phase::Docked(0));
    }

    #[test]
    fn a_lane_takes_you_to_the_next_region_and_a_course_follows() {
        let mut w = World::new(3);
        w.launch();
        w.plot(5);
        let first = w.course[0];
        assert_eq!(w.nav, Some(Nav::Mouth(first)));
        w.ships[0].pos = w.sector.mouth(0, first);
        let before = w.clock;
        w.jump();
        assert_eq!(w.region, first);
        assert!(w.clock >= before + LANE_HOURS);
        assert!(w.course.len() < 2);
    }

    #[test]
    fn trading_moves_credits_cargo_and_stock() {
        let mut w = World::new(3);
        let ice = 0;
        let got = w.buy(ice, 10);
        assert_eq!(got, 10);
        assert_eq!(w.cargo[ice], 10);
        assert!(w.credits < START_CREDITS);
        assert_eq!(w.sell(ice, 99), 10);
        assert_eq!(w.cargo_used(), 0);
        assert!(w.credits < START_CREDITS, "a round trip at one counter loses money");
    }

    #[test]
    fn the_hold_has_a_limit() {
        let mut w = World::new(3);
        w.credits = 1_000_000;
        assert_eq!(w.buy(0, 100), ship::TIIRA.cargo);
    }

    #[test]
    fn patrols_fight_pirates_and_leave_the_player_alone() {
        let mut w = World::new(11);
        w.launch();
        w.encounter_t = f32::INFINITY;
        w.spawn_traveller(Side::Patrol, Faction::Coop);
        assert_eq!(w.hostiles().count(), 0, "a friendly patrol is no threat");
        // put the patrol near the player, then bring in claim-jumpers
        let p = w.ships[0].pos;
        for s in w.ships.iter_mut().skip(1) {
            s.pos = p + v2(400.0, 0.0);
        }
        w.spawn_encounter();
        let pirates: Vec<u32> = w.hostiles().map(|s| s.id).collect();
        let hp0: f32 = w.hostiles().map(|s| s.hull + s.armour.iter().sum::<f32>() + s.shields.iter().sum::<f32>()).sum();
        fly(&mut w, 40.0);
        let hp1: f32 = w.ships.iter().filter(|s| pirates.contains(&s.id)).map(|s| s.hull + s.armour.iter().sum::<f32>() + s.shields.iter().sum::<f32>()).sum();
        assert!(hp1 < hp0, "the patrol (and only the patrol) wore the pirates down");
    }

    #[test]
    fn shooting_a_patrol_makes_enemies() {
        let mut w = World::new(11);
        w.launch();
        w.encounter_t = f32::INFINITY;
        w.spawn_traveller(Side::Patrol, Faction::Coop);
        let id = w.ships[1].id;
        let before = w.standing.of(Faction::Coop);
        let sh = Shot { pos: w.ships[1].pos, vel: v2(0.0, 100.0), owner: PLAYER, side: Side::Player, faction: Faction::Coop, damage: 1.0, left: 100.0 };
        w.hit(1, &sh);
        assert!(w.standing.of(Faction::Coop) < before);
        assert!(w.hostiles().any(|s| s.id == id), "it fights back now");
    }

    #[test]
    fn a_bribe_sends_them_away() {
        let mut w = World::demo(4);
        fly(&mut w, 5.0);
        let id = w.hostiles().next().map(|s| s.id).expect("pirates");
        w.ships[0].pos = w.ship(id).unwrap().pos + v2(2000.0, 0.0);
        let credits = w.credits;
        w.hail(id, 1);
        assert!(w.credits < credits);
        assert!(w.fleeing(id));
    }

    #[test]
    fn a_shot_hits_what_it_passes_through() {
        assert!(seg_hits(v2(0.0, 0.0), v2(100.0, 0.0), v2(50.0, 10.0), 20.0));
        assert!(!seg_hits(v2(0.0, 0.0), v2(100.0, 0.0), v2(150.0, 0.0), 20.0));
    }
}
