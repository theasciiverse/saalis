//! The simulation: the Northern Reach's belt regions, the bases and their
//! markets, the player's ship and hold, the clock, and the claim-jumpers.
//! No terminal code here (Torched's split), so it runs headless in tests and
//! in `--demo`.

pub mod ai;
pub mod body;
pub mod cue;
pub mod deck;
pub mod econ;
pub mod faction;
pub mod mission;
pub mod quest;
pub mod rng;
pub mod save;
pub mod shipyard;
pub mod sector;
pub mod ship;
pub mod v2;
pub mod war;

use ai::{Brain, Obstacle, Role};
use cue::Cue;
use econ::Market;
use faction::{Faction, Standing};
use mission::{Guild, Issuer, Mission, Task};
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
    /// a missile's exhaust
    Smoke,
}

/// A missile in flight.
#[derive(Clone, Debug)]
pub struct Missile {
    pub pos: V2,
    pub vel: V2,
    pub heading: f32,
    pub owner: u32,
    pub faction: Faction,
    /// what it homes on; none for a dumbfire or a broken lock
    pub target: Option<u32>,
    pub age: f32,
}

pub const MISSILE_SPEED: f32 = 650.0;
const MISSILE_TURN: f32 = 2.4;
const MISSILE_LIFE: f32 = 6.0;
const MISSILE_DAMAGE: f32 = 30.0;
/// A seeker locks on when the target is within this angle of the nose and
/// this range.
pub const LOCK_CONE: f32 = 0.6;
pub const LOCK_RANGE: f32 = 3000.0;

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
    pub black_market: bool,
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
    /// the player's ship is gone (or the player, aboard another)
    Lost,
    /// clamped onto disabled ship `id`, on foot aboard it
    Boarding(u32),
}

/// What a quest asks `World::spawn_tagged` for.
#[derive(Clone, Debug)]
pub struct Tagged {
    pub tag: String,
    pub class: &'static Class,
    pub side: Side,
    /// for patrols and traders; pirates are always claim-jumpers
    pub faction: Faction,
    pub count: u32,
    pub callsign: Option<String>,
    pub item: Option<String>,
    /// already knocked out, waiting to be boarded
    pub wreck: bool,
    pub skill: f32,
    /// how far from you they appear
    pub distance: f32,
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
    /// patrols that have scanned the player's hold already
    scanned: HashSet<u32>,
    /// who holds what, and the fleets on the lanes
    pub war: war::War,
    /// jobs taken
    pub missions: Vec<Mission>,
    /// units in the hold that belong to cargo jobs
    pub mission_cargo: u32,
    /// guild membership and rating, by `Guild::index`
    pub members: [bool; 2],
    pub rating: [u32; 2],
    /// what the boards offer at the base the player is docked at
    pub offers: Vec<Mission>,
    /// the fixer in this base's bar, if any: name, line, job
    pub fixer: Option<(String, String, Mission)>,
    /// you on foot: health, sidearm, vest, medkits
    pub body: body::Body,
    /// the ship you were killed aboard, if that is how it ended
    pub lost_aboard: Option<String>,
    /// what quests have handed you or you found for them, by name
    pub items: Vec<String>,
    pub next_mission: u32,
    /// what the player paid per unit, on average, for what is in the hold
    pub paid: [u32; econ::N],
    pub ships: Vec<Ship>,
    brains: HashMap<u32, Brain>,
    pub shots: Vec<Shot>,
    pub missiles: Vec<Missile>,
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
/// The chance a ship's last hit disables it instead of destroying it.
pub const DISABLE_CHANCE: f32 = 0.35;
/// How close (between hulls) and how slow (relative) you must be to board.
pub const BOARD_RANGE: f32 = 250.0;
pub const BOARD_SPEED: f32 = 40.0;

const MAYDAY: [&str; 3] = ["Mayday, mayday! Drives are gone!", "We're dead in space! Hold your fire!", "Reactor's scrammed. We're drifting."];

const KILLED: [&str; 3] = ["Bounty logged: {}.", "Confirmed. One {} less.", "Good kill. {} down."];

impl World {
    /// A new game: docked at Routa Station in Pallas Reach.
    pub fn new(seed: u64) -> World {
        let sector = Sector::reach();
        let markets = sector
            .all_bases()
            .iter()
            .map(|&(r, b)| {
                let d = &sector.regions[r].bases[b];
                Market::lawful(d.kind, sector.regions[r].owner, d.black_market)
            })
            .collect();
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
            scanned: HashSet::new(),
            war: war::War::new(&Sector::reach()),
            missions: Vec::new(),
            mission_cargo: 0,
            members: [false; 2],
            rating: [0; 2],
            offers: Vec::new(),
            fixer: None,
            body: body::Body::default(),
            lost_aboard: None,
            items: Vec::new(),
            next_mission: 0,
            paid: [0; econ::N],
            ships: Vec::new(),
            brains: HashMap::new(),
            shots: Vec::new(),
            missiles: Vec::new(),
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
        w.refresh_board();
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
            .map(|(i, b)| Station { name: b.name, kind: b.kind, black_market: b.black_market, pos: b.pos, radius: b.radius, market: all.iter().position(|&x| x == (r, i)).unwrap_or(0) })
            .collect();
        self.rocks = sector::rocks(self.seed, r, &self.sector).into_iter().map(|k| Rock { pos: k.pos, radius: k.radius, seed: k.seed }).collect();
        self.ships.retain(|s| s.id == PLAYER);
        self.brains.clear();
        self.shots.clear();
        self.missiles.clear();
        self.sparks.clear();
        self.blasts.clear();
        self.target = None;
        self.autopilot = false;
        let threat = self.war.threat(&self.sector, r);
        self.encounter_t = if threat == 0 { f32::INFINITY } else { self.rng.range(40.0, 110.0) / threat as f32 };
    }

    #[cfg(test)]
    pub fn enter_region_for_test(&mut self, r: usize) {
        self.enter_region(r);
    }

    #[cfg(test)]
    pub fn player_mut_for_test(&mut self) -> &mut Ship {
        self.player_mut().expect("the player's ship")
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
            && !s.disabled
            && s.id != PLAYER
            && s.side != Side::Trader
            && (s.side == Side::Pirate || self.angry.contains(&s.id) || self.standing.of(s.faction) <= faction::HOSTILE)
    }

    /// Who holds the current region now.
    pub fn owner(&self) -> Faction {
        self.war.control[self.region]
    }

    /// How dangerous the current region is now.
    pub fn threat(&self) -> u32 {
        self.war.threat(&self.sector, self.region)
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
        self.cargo.iter().sum::<u32>() + self.mission_cargo
    }

    pub fn cargo_room(&self) -> u32 {
        self.ships.first().map_or(0, |s| s.cargo_cap()).saturating_sub(self.cargo_used())
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
            for h in 0..(whole as u32).min(48) {
                let news = self.war.hour(&self.sector, &mut self.rng, before + 1.0 + h as f64, self.region);
                self.war_news(news);
            }
        }
    }

    // --- missions ------------------------------------------------------

    /// The boards and the bar at the base the player is docked at.
    pub fn refresh_board(&mut self) {
        let Phase::Docked(b) = self.phase else { return };
        let bd = mission::board(self.seed, &self.sector, self.region, b, self.clock, self.members, self.rating, &mut self.next_mission);
        // jobs already taken stay off the board
        let taken: Vec<Task> = self.missions.iter().map(|m| m.task.clone()).collect();
        self.offers = bd.offers.into_iter().filter(|m| !taken.contains(&m.task)).collect();
        self.fixer = bd.fixer.filter(|f| !taken.contains(&f.2.task));
    }

    /// Takes a job; says why not if it can't.
    fn take(&mut self, m: Mission) -> Result<String, String> {
        if self.missions.len() >= mission::MAX_ACTIVE {
            return Err(format!("You can carry {} jobs at most.", mission::MAX_ACTIVE));
        }
        if m.units() > self.cargo_room() {
            return Err(format!("It needs {} units of hold space.", m.units()));
        }
        self.mission_cargo += m.units();
        let line = format!("Taken: {}.", m.describe(&self.sector));
        log::info!("mission taken {:?}", m);
        // point the way
        if m.region() != self.region && self.course.is_empty() {
            self.plot(m.region());
        }
        self.missions.push(m);
        Ok(line)
    }

    /// A: takes board offer `i`.
    pub fn accept_offer(&mut self, i: usize) -> Result<String, String> {
        let Some(m) = self.offers.get(i).cloned() else { return Err("Nothing there.".into()) };
        let line = self.take(m)?;
        self.offers.remove(i);
        Ok(line)
    }

    /// Takes the fixer's job.
    pub fn accept_fixer(&mut self) -> Result<String, String> {
        let Some((_, _, m)) = self.fixer.clone() else { return Err("Nobody's buying today.".into()) };
        let line = self.take(m)?;
        self.fixer = None;
        Ok(line)
    }

    /// Joins a guild: the fee, and the faction must not dislike you.
    pub fn join(&mut self, g: Guild) -> Result<String, String> {
        if self.members[g.index()] {
            return Err(format!("You're already in the {}.", g.name()));
        }
        if self.standing.of(g.faction()) < 0 {
            return Err(format!("The {} won't have you.", g.name()));
        }
        if self.credits < g.fee() {
            return Err(format!("Membership is {} cr.", g.fee()));
        }
        self.credits -= g.fee();
        self.members[g.index()] = true;
        self.refresh_board();
        log::info!("joined {}", g.name());
        Ok(format!("Welcome to the {}. The whole board is yours.", g.name()))
    }

    fn complete(&mut self, id: u32) {
        let Some(i) = self.missions.iter().position(|m| m.id == id) else { return };
        let m = self.missions.remove(i);
        self.mission_cargo = self.mission_cargo.saturating_sub(m.units());
        self.credits += m.pay;
        if let Issuer::Guild(g) = m.issuer {
            self.rating[g.index()] += 1;
            self.standing.shift(g.faction(), 4);
        }
        log::info!("mission done {:?} pay {}", m.task, m.pay);
        let from = m.issuer_name();
        self.say(from, format!("Job done: {}. {} cr paid.", m.describe(&self.sector), m.pay), None);
        self.cues.push(Cue::Target);
    }

    fn fail(&mut self, id: u32, why: &str) {
        let Some(i) = self.missions.iter().position(|m| m.id == id) else { return };
        let m = self.missions.remove(i);
        self.mission_cargo = self.mission_cargo.saturating_sub(m.units());
        if let Issuer::Guild(g) = m.issuer {
            self.rating[g.index()] = self.rating[g.index()].saturating_sub(1);
            self.standing.shift(g.faction(), -6);
        }
        log::info!("mission failed {:?}: {why}", m.task);
        let from = m.issuer_name();
        self.say(from, format!("{why} {}: the job is off.", m.describe(&self.sector)), None);
    }

    /// Deadlines, patrol points and finished bounties.
    fn check_missions(&mut self) {
        let late: Vec<u32> = self.missions.iter().filter(|m| self.clock > m.deadline).map(|m| m.id).collect();
        for id in late {
            self.fail(id, "Too late.");
        }
        if let Some(p) = self.player().map(|p| p.pos) {
            let region = self.region;
            let mouths: Vec<(usize, V2)> = self.sector.neighbours(region).into_iter().map(|to| (to, self.sector.mouth(region, to))).collect();
            for m in &mut self.missions {
                if let Task::Patrol { region: r, mouths: list, seen } = &mut m.task {
                    if *r == region {
                        for (k, to) in list.iter().enumerate() {
                            if !seen[k] && mouths.iter().any(|&(t, at)| t == *to && (at - p).len() < MOUTH_RADIUS) {
                                seen[k] = true;
                                log::info!("patrol point {k} seen");
                            }
                        }
                    }
                }
            }
        }
        let done: Vec<u32> = self
            .missions
            .iter()
            .filter(|m| match &m.task {
                Task::Patrol { seen, .. } => seen.iter().all(|x| *x),
                Task::Bounty { need, done, .. } => done >= need,
                _ => false,
            })
            .map(|m| m.id)
            .collect();
        for id in done {
            self.complete(id);
        }
    }

    /// Units of contraband aboard under a faction's law.
    pub fn contraband(&self, f: Faction) -> u32 {
        (0..econ::N).filter(|&g| econ::illegal(f, g)).map(|g| self.cargo[g]).sum()
    }

    /// Patrols of factions with laws scan the player's hold once each, when
    /// they pass within 1.2 km.
    fn patrol_scans(&mut self) {
        let Some(me) = self.player().map(|p| p.pos) else { return };
        let due: Vec<(u32, Faction, String)> = self
            .ships
            .iter()
            .filter(|s| s.alive && s.side == Side::Patrol && !self.scanned.contains(&s.id) && !self.hostile_to_player(s) && (s.pos - me).len() < 1200.0)
            .filter(|s| (0..econ::N).any(|g| econ::illegal(s.faction, g)))
            .map(|s| (s.id, s.faction, s.callsign.clone()))
            .collect();
        for (id, f, from) in due {
            self.scanned.insert(id);
            self.inspect(f, from, Some(id));
        }
    }

    /// A customs inspection under faction `f`'s law. What isn't hidden is
    /// found; hidden compartments are found more often the more of the hold
    /// they take (Ascii Sector: under a tenth is safe). Found goods are
    /// seized with a fine; a fine you can't pay makes a patrol shoot.
    fn inspect(&mut self, f: Faction, from: String, patrol: Option<u32>) {
        let illegal = self.contraband(f);
        if illegal == 0 {
            if patrol.is_some() {
                self.say(from, "Scan complete. You're clean. Fly safe.", Some(Side::Patrol));
            }
            return;
        }
        let Some(p) = self.player() else { return };
        let hidden = p.kit.hidden;
        let cap = p.cargo_cap().max(1) as f32;
        let visible = illegal.saturating_sub(hidden);
        let p_hidden = ((hidden as f32 / cap - 0.1) * 2.0).clamp(0.05, 0.6);
        let found = visible > 0 || self.rng.chance(p_hidden);
        log::info!("inspection by {:?}: contraband={illegal} hidden={hidden} found={found}", f);
        if !found {
            self.say(from, "Scan complete. You're clean. Fly safe.", Some(Side::Patrol));
            return;
        }
        let mut value = 0;
        for g in (0..econ::N).filter(|&g| econ::illegal(f, g)) {
            value += self.cargo[g] * econ::index(&self.markets, g);
            self.cargo[g] = 0;
            self.paid[g] = 0;
        }
        let fine = value / 2;
        self.standing.shift(f, -8);
        if self.credits >= fine {
            self.credits -= fine;
            self.say(from, format!("Contraband aboard. Seized, and a {fine} cr fine."), Some(Side::Patrol));
        } else {
            self.credits = 0;
            self.standing.shift(f, -10);
            self.say(from, "Contraband, and you can't pay the fine. Heave to!", Some(Side::Patrol));
            if let Some(id) = patrol {
                self.angry.insert(id);
            }
        }
        self.cues.push(Cue::Warning);
    }

    /// The war's news over comms, and a battle where the player is.
    fn war_news(&mut self, news: Vec<war::News>) {
        for n in news {
            log::info!("war: {n:?}");
            let name = |r: usize| self.sector.regions[r].name;
            let line = match n {
                war::News::Sighted { faction, to } if to == self.region || self.sector.neighbours(self.region).contains(&to) => {
                    format!("{} fleet on the lanes, bound for {}.", faction.name(), name(to))
                }
                war::News::Taken { region, by, from } => format!("{} has fallen to the {}. The {} are out.", name(region), by.name(), from.name()),
                war::News::Held { region, against } if region == self.region => format!("The {} were beaten off at {}.", against.name(), name(region)),
                war::News::Battle { region } if self.phase == Phase::Flying && region == self.region => {
                    // the fleet comes out of the lane near the player
                    self.spawn_encounter();
                    if self.owner() != Faction::Pirates {
                        let owner = self.owner();
                        self.spawn_traveller(Side::Patrol, owner);
                        let me = self.player().map(|p| p.pos).unwrap_or_default();
                        let n = self.ships.len();
                        for s in self.ships.iter_mut().skip(n.saturating_sub(2)) {
                            s.pos = me + v2(600.0, 300.0);
                        }
                    }
                    "Fleet action in this region! All ships, weapons free.".to_string()
                }
                _ => continue,
            };
            self.say("Lane beacon", line, None);
        }
    }

    /// A bounty job is open in the current region.
    fn hunting_here(&self) -> bool {
        self.missions.iter().any(|m| matches!(m.task, Task::Bounty { region, .. } if region == self.region))
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
        let mut note = format!("{}: {}.", def.name, def.note);
        if self.war.control[to] != def.owner {
            note.push_str(&format!(" Held by the {} now.", self.war.control[to].name()));
        }
        let threat = self.threat();
        self.say("Lane beacon", note, None);
        if (threat > 0 && self.rng.chance(0.2 * threat as f32)) || self.hunting_here() {
            self.encounter_t = self.rng.range(3.0, 8.0);
        }
        self.check_missions();
    }

    /// L: land at the nearest base, if close and slow enough.
    pub fn land(&mut self) {
        if self.phase != Phase::Flying {
            return;
        }
        // L beside a disabled target boards it
        if self.try_board() {
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
            p.shields = [p.max_shield(); 4];
            p.energy = p.max_energy();
            p.fuel = p.class.fuel;
        }
        // a night ashore heals you
        self.body.hp = body::MAX_HP;
        self.ships.retain(|s| s.id == PLAYER);
        self.brains.clear();
        self.shots.clear();
        if self.nav == Some(Nav::Base(b)) && self.course.is_empty() {
            self.nav = None;
        }
        self.pass_hours(0.5);
        log::info!("docked at {}", self.bases[b].name);
        if !self.bases[b].black_market {
            let owner = self.owner();
            let from = format!("{} Customs", self.bases[b].name.split(' ').next().unwrap_or("Base"));
            self.inspect(owner, from, None);
        }
        self.check_missions();
        // deliveries
        let here = (self.region, b);
        let done: Vec<u32> = self
            .missions
            .iter()
            .filter(|m| matches!(m.task, Task::Cargo { region, base, .. } | Task::Courier { region, base } if (region, base) == here))
            .map(|m| m.id)
            .collect();
        for id in done {
            self.complete(id);
        }
        self.refresh_board();
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

    // --- boarding ----------------------------------------------------

    /// L with a disabled ship targeted: clamp on and board it, if close and
    /// slow enough. Returns whether L was about boarding.
    pub fn try_board(&mut self) -> bool {
        let Some(t) = self.target.and_then(|id| self.ship(id)).filter(|s| s.disabled).cloned() else { return false };
        let Some(p) = self.player() else { return false };
        let gap = (t.pos - p.pos).len() - t.class.radius - p.class.radius;
        let rel = (t.vel - p.vel).len();
        if gap > BOARD_RANGE {
            self.say_note("Boarding", &format!("Close within {} m of the wreck to clamp on.", BOARD_RANGE as u32));
        } else if rel > BOARD_SPEED {
            self.say_note("Boarding", "Match its drift first (M).");
        } else if self.hostiles_near(2500.0) {
            self.say_note("Boarding", "Not with hostiles about.");
        } else {
            self.phase = Phase::Boarding(t.id);
            self.phase_t = 0.0;
            self.autopilot = false;
            if let Some(p) = self.player_mut() {
                p.vel = t.vel;
                p.set_speed = 0.0;
            }
            // boarding a lawful ship is piracy, whoever knocked it out
            if t.side != Side::Pirate {
                self.standing.shift(t.faction, -10);
            }
            log::info!("boarding {} ({})", t.callsign, t.class.name);
        }
        true
    }

    /// What is aboard ship `id`, for the deck to lay out.
    pub fn boarding(&mut self, id: u32) -> Option<deck::Boarding> {
        let s = self.ship(id)?.clone();
        let size = match s.class.size {
            ship::Size::Light => 0,
            ship::Size::Medium => 1,
            ship::Size::Heavy => 2,
        };
        let crew_kind = match s.side {
            Side::Pirate => 0,
            Side::Patrol => 2,
            _ => 1,
        };
        // one at the controls, one more per size; a heavy hull may carry a third
        let crew = 1 + size + u8::from(size == 2 && self.rng.chance(0.5));
        let lots = match (s.side, size) {
            (Side::Trader, _) => 2 + self.rng.below(3),
            (_, 0) => 1,
            _ => 1 + self.rng.below(2),
        };
        let mut cargo = Vec::new();
        for _ in 0..lots {
            // raiders carry what they took, and what they sell on the side
            let g = if s.side == Side::Pirate && self.rng.chance(0.5) { 10 + self.rng.below(2) as usize } else { self.rng.below(10) as usize };
            cargo.push((g, 3 + self.rng.below(10)));
        }
        let credits = match s.side {
            Side::Pirate => 150 + self.rng.below(550),
            Side::Trader => 50 + self.rng.below(250),
            _ => self.rng.below(100),
        };
        Some(deck::Boarding {
            size,
            faction: s.faction,
            crew_kind,
            crew,
            cargo,
            credits,
            item: s.item.clone(),
        })
    }

    /// Takes up to `n` of good `g` into the hold; returns how many fitted.
    pub fn take_cargo(&mut self, g: usize, n: u32) -> u32 {
        let got = n.min(self.cargo_room());
        if got > 0 {
            let held = self.cargo[g];
            // free goods bring the average paid down
            self.paid[g] = ((self.paid[g] * held) as f32 / (held + got) as f32).round() as u32;
            self.cargo[g] += got;
            log::info!("took {got} {} aboard", econ::GOODS[g].name);
        }
        got
    }

    /// Back aboard your own ship: cast off, and the wreck is scuttled.
    pub fn leave_boarded(&mut self) {
        let Phase::Boarding(id) = self.phase else { return };
        self.phase = Phase::Flying;
        self.phase_t = 0.0;
        if let Some(i) = self.ships.iter().position(|s| s.id == id) {
            let (pos, vel) = (self.ships[i].pos, self.ships[i].vel);
            if let Some(p) = self.player_mut() {
                let away = (p.pos - pos).norm();
                p.pos = pos + away * 400.0;
                p.vel = vel + away * 60.0;
            }
            self.ships[i].alive = false;
            self.destroyed(i, PLAYER);
        }
        self.say_note("Boarding", "You cast off. Behind you the wreck's reactor lets go.");
        log::info!("left the boarded ship");
    }

    /// `--board`: launched, with a raider knocked out alongside and the
    /// clamps already on.
    pub fn practice_boarding(&mut self) {
        self.launch();
        self.spawn_encounter();
        self.ships.truncate(2);
        let Some(me) = self.player().map(|p| p.pos) else { return };
        if self.ships.len() < 2 {
            return;
        }
        self.ships[1].pos = me + v2(0.0, -150.0);
        self.ships[1].vel = V2::ZERO;
        self.ships[0].vel = V2::ZERO;
        while !self.knock_out(1, 0) {}
        self.target = Some(self.ships[1].id);
        self.comms.clear();
        self.try_board();
    }

    /// Killed on foot aboard someone else's ship.
    pub fn die_aboard(&mut self) {
        let Phase::Boarding(id) = self.phase else { return };
        self.lost_aboard = Some(self.ship(id).map_or("a wreck".into(), |s| s.class.name.to_string()));
        self.phase = Phase::Lost;
        self.phase_t = 0.0;
        self.cues.push(Cue::PlayerLost);
        log::info!("killed aboard {:?}", self.lost_aboard);
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
        if !self.markets[m].open[g] {
            return 0;
        }
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
        let armour: f32 = p.max_armour().iter().zip(p.armour).map(|(m, a)| m - a).sum();
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
            let max = p.max_armour();
            for (a, m) in p.armour.iter_mut().zip(max) {
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
    pub fn rumour(&mut self) -> String {
        let here = match self.threat() {
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
    pub fn tip(&mut self) -> String {
        let g = self.rng.below(econ::N as u32) as usize;
        let all = self.sector.all_bases();
        let best = (0..self.markets.len()).filter(|&m| self.markets[m].open[g]).max_by_key(|&m| self.markets[m].sell_price(g));
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
        if let Phase::Docked(_) | Phase::Boarding(_) = self.phase {
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
            // a wreck drifts, slowing, turning over
            if s.disabled {
                s.pos += s.vel * dt;
                s.vel = s.vel * (1.0 - 0.3 * dt);
                s.heading += 0.4 * dt;
                continue;
            }
            if s.fly(it, dt) && s.id == PLAYER {
                self.cues.push(Cue::Burner);
            }
            if it.fire {
                if let Some(g) = s.ready_gun() {
                    let gun = s.kit.guns[g];
                    s.cooldown[g] = gun.refire;
                    s.energy -= gun.energy;
                    s.next_gun = (g + 1) % s.kit.guns.len();
                    let n = s.kit.guns.len() as f32;
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
                } else if s.id == PLAYER && s.kit.guns.first().is_some_and(|g| s.energy < g.energy) && self.phase_t > 0.2 {
                    self.cues.push(Cue::Empty);
                }
            }
        }

        self.launches(&intents);
        self.turrets();
        self.fly_missiles(dt);
        self.move_shots(dt);
        self.collide();
        self.age_effects(dt);
        self.tidy();
        self.encounter(dt);
        if (self.time * 2.0).floor() != ((self.time - dt) * 2.0).floor() {
            self.check_missions();
            self.patrol_scans();
        }
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
        let threat = self.threat() as f32;
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

    /// Whether ship `s` has a seeker lock on ship `t`.
    pub fn locked(&self, s: &Ship, t: &Ship) -> bool {
        let to = t.pos - s.pos;
        to.len() < LOCK_RANGE && v2::turn_to(s.heading, to.heading()).abs() < LOCK_CONE
    }

    /// Missiles away, for every ship that asked and can.
    fn launches(&mut self, intents: &[Intent]) {
        let mut away = Vec::new();
        for (i, it) in intents.iter().enumerate() {
            let s = &self.ships[i];
            if !it.missile || !s.alive || !s.kit.launcher || s.launch_cd > 0.0 || s.kit.dumb + s.kit.seekers == 0 {
                continue;
            }
            // the player's target, or for others what their pilot is after
            let target = if s.id == PLAYER { self.target } else { self.brains.get(&s.id).and_then(|b| b.target) };
            let lock = target.and_then(|t| self.ship(t)).filter(|t| s.kit.seekers > 0 && self.locked(s, t)).map(|t| t.id);
            if lock.is_none() && s.kit.dumb == 0 {
                continue;
            }
            away.push((i, lock));
        }
        for (i, lock) in away {
            let s = &mut self.ships[i];
            if lock.is_some() {
                s.kit.seekers -= 1;
            } else {
                s.kit.dumb -= 1;
            }
            s.launch_cd = 1.2;
            let fwd = s.forward();
            self.missiles.push(Missile { pos: s.pos + fwd * (s.class.radius + 10.0), vel: s.vel + fwd * 150.0, heading: s.heading, owner: s.id, faction: s.faction, target: lock, age: 0.0 });
            log::info!("missile from {} lock={:?}", s.callsign, lock);
            self.cues.push(Cue::Missile);
            if lock == Some(PLAYER) {
                self.cues.push(Cue::Warning);
                self.say_note("Threat warning", "Missile locked on!");
            }
        }
    }

    /// Rear turrets: a laser at the nearest enemy behind, within 900 m.
    fn turrets(&mut self) {
        let mut shots = Vec::new();
        for s in self.ships.iter().filter(|s| s.alive && !s.disabled && s.kit.turret && s.turret_cd <= 0.0) {
            let enemy = self
                .ships
                .iter()
                .filter(|o| {
                    let to = o.pos - s.pos;
                    to.len() < 900.0 && v2::turn_to(s.heading, to.heading()).abs() > 1.6 && fights_of(&self.standing, &self.angry, s, o)
                })
                .min_by(|a, b| (a.pos - s.pos).len().total_cmp(&(b.pos - s.pos).len()));
            if let Some(t) = enemy {
                let aim = ai::lead(s.pos, s.vel, t.pos, t.vel, ship::LASER.speed);
                let dir = (aim - s.pos).norm();
                shots.push((s.id, Shot { pos: s.pos + dir * s.class.radius, vel: s.vel + dir * ship::LASER.speed, owner: s.id, side: s.side, faction: s.faction, damage: ship::LASER.damage * 0.8, left: 900.0 }));
            }
        }
        for (id, sh) in shots {
            if let Some(s) = self.ships.iter_mut().find(|s| s.id == id) {
                s.turret_cd = 0.4;
            }
            self.shots.push(sh);
        }
    }

    /// Missiles steer toward their lock, which ECM may break; they burst on
    /// any ship but their owner's side, or burn out.
    fn fly_missiles(&mut self, dt: f32) {
        let mut missiles = std::mem::take(&mut self.missiles);
        let mut hits = Vec::new();
        missiles.retain_mut(|m| {
            m.age += dt;
            let tracked = m.target.and_then(|t| self.ship(t)).map(|t| (t.pos, t.kit.ecm));
            if let Some((tpos, ecm)) = tracked {
                // ECM: a chance a second to break the lock
                if ecm > 0 && self.rng.chance(0.25 * ecm as f32 * dt) {
                    log::info!("ECM broke a lock on {:?}", m.target);
                    m.target = None;
                } else {
                    let want = (tpos - m.pos).heading();
                    m.heading += v2::turn_to(m.heading, want).clamp(-MISSILE_TURN * dt, MISSILE_TURN * dt);
                }
            }
            let speed = m.vel.len().max(MISSILE_SPEED.min(m.vel.len() + 900.0 * dt));
            m.vel = V2::from_heading(m.heading) * speed.min(MISSILE_SPEED + 300.0);
            let from = m.pos;
            m.pos += m.vel * dt;
            let hit = self.ships.iter().position(|s| s.alive && s.id != m.owner && !(m.owner != PLAYER && s.id != PLAYER && s.faction == m.faction) && seg_hits(from, m.pos, s.pos, s.class.radius + 8.0));
            if let Some(i) = hit {
                hits.push((i, m.clone()));
                return false;
            }
            m.age < MISSILE_LIFE
        });
        for m in &missiles {
            if self.rng.chance(0.5) {
                self.sparks.push(Spark { pos: m.pos - m.vel.norm() * 15.0, vel: m.vel * -0.1, age: 0.0, life: 0.6, kind: SparkKind::Smoke });
            }
        }
        self.missiles = missiles;
        for (i, m) in hits {
            self.blasts.push(Blast { pos: m.pos, vel: V2::ZERO, radius: 40.0, age: 0.0, life: 0.4 });
            let sh = Shot { pos: m.pos, vel: m.vel, owner: m.owner, side: if m.owner == PLAYER { Side::Player } else { Side::Pirate }, faction: m.faction, damage: MISSILE_DAMAGE, left: 0.0 };
            if self.ships[i].alive {
                self.hit(i, &sh);
            }
        }
    }

    /// A missile is homing on the player.
    pub fn missile_warning(&self) -> bool {
        self.missiles.iter().any(|m| m.target == Some(PLAYER))
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
        if struck == Struck::Destroyed && !self.knock_out(i, sh.owner) {
            self.destroyed(i, sh.owner);
        }
    }

    /// Sometimes a ship's last hit leaves it dead in space, not blown apart:
    /// it drifts, can't fight, and can be boarded. Returns whether it did.
    /// It counts as a kill now; blowing it up later pays nothing more.
    pub fn knock_out(&mut self, i: usize, by: u32) -> bool {
        let s = &self.ships[i];
        // a ship carrying a quest's item is always left to be boarded
        if s.id == PLAYER || s.disabled || !(s.item.is_some() || self.rng.chance(DISABLE_CHANCE)) {
            return false;
        }
        let s = &mut self.ships[i];
        s.alive = true;
        s.disabled = true;
        s.hull = 1.0;
        s.shields = [0.0; 4];
        s.energy = 0.0;
        s.set_speed = 0.0;
        s.burning = false;
        let s = s.clone();
        self.brains.remove(&s.id);
        self.angry.remove(&s.id);
        log::info!("disabled {} ({}) by {by}", s.callsign, s.class.name);
        self.spray(s.pos, s.vel * 0.3, 18, SparkKind::Smoke);
        self.cues.push(Cue::Kill);
        let line = MAYDAY[self.rng.below(MAYDAY.len() as u32) as usize];
        self.say(s.callsign.clone(), line, Some(s.side));
        self.credit(&s, by);
        true
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
        // a wreck already counted when it was knocked out
        if !s.disabled {
            self.credit(&s, by);
        }
    }

    /// The bounty, standing and mission credit for a ship taken out.
    fn credit(&mut self, s: &Ship, by: u32) {
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
            self.war.pirate_down(self.region);
            let region = self.region;
            for m in &mut self.missions {
                if let Task::Bounty { region: r, done, .. } = &mut m.task {
                    if *r == region {
                        *done += 1;
                    }
                }
            }
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
            SparkKind::Rock | SparkKind::Smoke => 90.0,
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
            // a ship carrying a quest's item stays until it's dealt with
            if s.id == PLAYER || !s.alive || s.item.is_some() {
                continue;
            }
            let far = player_pos.is_some_and(|p| (s.pos - p).len() > if s.side == Side::Pirate { LEAVE_RANGE } else { LEAVE_RANGE * 2.0 });
            if far || arrived.contains(&s.id) {
                s.alive = false;
                log::info!("{} left the region", s.callsign);
            }
        }
        self.angry.retain(|id| self.ships.iter().any(|s| s.id == *id && s.alive));
        self.scanned.retain(|id| self.ships.iter().any(|s| s.id == *id && s.alive));
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
        let threat = self.threat() as f32;
        self.encounter_t = if self.busy { 8.0 } else { self.rng.range(35.0, 80.0) };
        if self.busy {
            self.spawn_encounter();
            return;
        }
        let owner = self.owner();
        let patrol = if owner == Faction::Pirates { 0.0 } else { 1.2 };
        let trade = if self.bases.is_empty() { 0.4 } else { 1.4 };
        let roll = self.rng.range(0.0, threat + patrol + trade);
        if roll < threat || self.hunting_here() {
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
            s.kit.turret = ship_side == Side::Trader;
            s.kit.launcher = ship_side == Side::Patrol;
            s.kit.seekers = if ship_side == Side::Patrol { 2 } else { 0 };
            s.vel = V2::from_heading(heading) * class.max_speed * 0.6;
            let goal = to + (pos - start);
            self.brains.insert(id, Brain::new(0.6, if role == Role::Trader { 1.0 } else { 0.25 }).with_role(role, Some(goal)));
            self.ships.push(s);
        }
        log::info!("{:?} {:?} passing through {}", side, faction, self.region_name());
    }

    /// Ships a quest asks for, near you; returns how many came. Only while
    /// flying: docking and lanes clear the field.
    pub fn spawn_tagged(&mut self, q: &Tagged) -> u32 {
        if self.phase != Phase::Flying {
            return 0;
        }
        let Some(me) = self.player() else { return 0 };
        let centre = me.pos;
        let bearing = self.rng.range(-3.14, 3.14);
        let at = centre + V2::from_heading(bearing) * q.distance;
        let n = q.count.clamp(1, 8);
        for k in 0..n {
            let id = self.next_id;
            self.next_id += 1;
            let offset = V2::from_heading(bearing + 1.57) * ((k as f32 - n as f32 / 2.0) * 180.0);
            let pos = at + offset;
            let heading = (centre - pos).heading();
            let callsign = match &q.callsign {
                Some(c) if n == 1 => c.clone(),
                Some(c) => format!("{c} {}", k + 1),
                None => format!("{} {}", q.class.name, callsign(&mut self.rng)),
            };
            let mut s = Ship::new(id, q.class, q.side, callsign, pos, heading);
            s.faction = if q.side == Side::Pirate {
                Faction::Pirates
            } else {
                q.faction
            };
            s.tag = Some(q.tag.clone());
            // the item rides with the first of them
            s.item = if k == 0 { q.item.clone() } else { None };
            if q.wreck {
                // dead in space already: drifting, open to boarding
                s.vel = V2::from_heading(heading) * 5.0;
                s.set_speed = 0.0;
                s.hull = 1.0;
                s.disabled = true;
                self.ships.push(s);
                continue;
            }
            s.vel = V2::from_heading(heading) * q.class.max_speed * 0.6;
            s.kit.launcher = q.class.size != Size::Light && q.side != Side::Trader;
            s.kit.seekers = if s.kit.launcher { 2 } else { 0 };
            s.kit.turret = q.class.size == Size::Heavy || q.side == Side::Trader;
            let brain = match q.side {
                Side::Trader => {
                    let goal = self.navs().first().map(|&n| self.nav_pos(n));
                    Brain::new(q.skill, 1.0).with_role(Role::Trader, goal)
                }
                Side::Patrol => Brain::new(q.skill, 0.25).with_role(Role::Fighter, Some(centre)),
                _ => Brain::new(
                    q.skill,
                    if q.class.size == Size::Heavy {
                        0.0
                    } else {
                        0.3
                    },
                ),
            };
            self.brains.insert(id, brain);
            self.ships.push(s);
        }
        if q.side == Side::Pirate && !q.wreck {
            self.cues.push(Cue::Contact);
        }
        log::info!(
            "quest ships '{}': {n} {} ({:?}) in {}",
            q.tag,
            q.class.name,
            q.side,
            self.region_name()
        );
        n
    }

    /// Ships with a quest's tag: (still flying or drifting, knocked out).
    pub fn tagged(&self, tag: &str) -> (u32, u32) {
        let mine = self
            .ships
            .iter()
            .filter(|s| s.alive && s.tag.as_deref() == Some(tag));
        mine.fold((0, 0), |(a, d), s| (a + 1, d + u32::from(s.disabled)))
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
        let threat = self.threat();
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
            if class.size != Size::Light {
                s.kit.launcher = true;
                s.kit.seekers = 2;
            }
            s.kit.turret = class.size == Size::Heavy;
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
    if !a.alive || !b.alive || a.disabled || b.disabled || a.id == b.id {
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
    fn a_courier_job_pays_on_landing_and_a_late_one_fails() {
        let mut w = World::new(8);
        w.members = [true; 2];
        w.refresh_board();
        let i = w.offers.iter().position(|m| matches!(m.task, Task::Courier { .. } | Task::Cargo { .. })).expect("a traders' job");
        let m = w.offers[i].clone();
        let units = m.units();
        w.accept_offer(i).expect("taken");
        assert_eq!(w.cargo_used(), units);
        // fly there by teleport
        let (Task::Courier { region, base } | Task::Cargo { region, base, .. }) = m.task else { unreachable!() };
        w.launch();
        w.enter_region(region);
        w.ships[0].pos = w.bases[base].pos;
        let credits = w.credits;
        w.dock(base);
        assert!(w.missions.is_empty(), "delivered");
        assert_eq!(w.credits, credits + m.pay);
        assert_eq!(w.cargo_used(), 0);
        assert_eq!(w.rating[0], 1);

        // and one left too long
        let mut w = World::new(8);
        w.members = [true; 2];
        w.refresh_board();
        w.accept_offer(0).expect("taken");
        let before = w.standing.of(Faction::Traders);
        w.clock += 24.0 * 30.0;
        w.check_missions();
        assert!(w.missions.is_empty());
        assert!(w.standing.of(w.offers.first().map_or(Faction::Traders, |_| Faction::Traders)) <= before);
    }

    #[test]
    fn a_bounty_counts_kills_in_its_region() {
        let mut w = World::demo(8);
        w.missions.push(Mission { id: 99, issuer: Issuer::Guild(Guild::Watch), task: Task::Bounty { region: 0, need: 1, done: 0 }, pay: 500, deadline: 1e9 });
        fly(&mut w, 5.0);
        let i = w.ships.iter().position(|s| s.side == Side::Pirate).expect("a pirate");
        let credits = w.credits;
        w.ships[i].hull = 0.1;
        w.ships[i].shields = [0.0; 4];
        w.ships[i].armour = [0.0; 4];
        let sh = Shot { pos: w.ships[i].pos, vel: v2(0.0, 100.0), owner: PLAYER, side: Side::Player, faction: Faction::Coop, damage: 50.0, left: 100.0 };
        w.hit(i, &sh);
        w.check_missions();
        assert!(w.missions.is_empty());
        assert!(w.credits >= credits + 500);
    }

    #[test]
    fn joining_a_guild_costs_and_opens_the_board() {
        let mut w = World::new(8);
        let n = w.offers.len();
        w.join(Guild::Traders).expect("joined");
        assert_eq!(w.credits, START_CREDITS - Guild::Traders.fee());
        assert!(w.offers.len() > n);
        assert!(w.join(Guild::Traders).is_err());
    }

    #[test]
    fn a_knocked_out_pirate_counts_once_and_can_be_boarded_and_left() {
        let mut w = World::new(3);
        w.launch();
        w.encounter_t = f32::INFINITY;
        w.busy = false;
        w.spawn_encounter();
        w.ships.truncate(2);
        let id = w.ships[1].id;
        assert_eq!(w.ships[1].side, Side::Pirate);
        let me = w.ships[0].pos;
        w.ships[1].pos = me + v2(0.0, -300.0);
        w.ships[1].vel = V2::ZERO;
        w.ships[0].vel = V2::ZERO;
        w.ships[0].set_speed = 0.0;
        let credits = w.credits;
        // knocked out until the dice say so
        while !w.knock_out(1, PLAYER) {}
        assert!(w.ships[1].disabled && w.ships[1].alive);
        assert_eq!(w.kills, 1);
        let paid = w.credits - credits;
        assert!(paid > 0, "the bounty is paid on knocking out");
        assert!(w.hostiles().next().is_none(), "a wreck is no threat");
        fly(&mut w, 1.0);
        assert!(w.ship(id).is_some(), "it drifts on; nobody shoots it");
        // too far: L says so and does not board
        w.ships[1].pos = me + v2(0.0, -3000.0);
        w.target = Some(id);
        w.land();
        assert_eq!(w.phase, Phase::Flying);
        w.ships[1].pos = w.ships[0].pos + v2(0.0, -150.0);
        w.ships[1].vel = w.ships[0].vel;
        w.land();
        assert_eq!(w.phase, Phase::Boarding(id));
        let b = w.boarding(id).unwrap();
        assert_eq!(b.crew_kind, 0, "raiders");
        let room = w.cargo_room();
        assert_eq!(w.take_cargo(0, room + 5), room, "only what fits");
        w.leave_boarded();
        assert_eq!(w.phase, Phase::Flying);
        assert!(w.ship(id).is_none(), "scuttled");
        assert_eq!(w.kills, 1, "and not counted twice");
        assert_eq!(w.credits - credits, paid);
    }

    #[test]
    fn dying_aboard_ends_the_game() {
        let mut w = World::new(3);
        w.launch();
        w.encounter_t = f32::INFINITY;
        w.spawn_encounter();
        let id = w.ships[1].id;
        w.ships[1].disabled = true;
        w.phase = Phase::Boarding(id);
        w.die_aboard();
        assert_eq!(w.phase, Phase::Lost);
        assert!(w.lost_aboard.is_some());
    }

    #[test]
    fn a_seeker_finds_a_locked_target() {
        let run = |ecm: u8| {
            let mut w = World::new(3);
            w.launch();
            w.encounter_t = f32::INFINITY;
            w.busy = false;
            w.spawn_encounter();
            let id = w.ships[1].id;
            let me = w.ships[0].pos;
            {
                let t = &mut w.ships[1];
                t.pos = me + v2(0.0, -1500.0);
                t.vel = V2::ZERO;
                t.kit.ecm = ecm;
                t.shields = [0.0; 4];
            }
            w.brains.clear();
            w.ships[0].heading = 0.0;
            w.ships[0].kit.launcher = true;
            w.ships[0].kit.seekers = 1;
            w.target = Some(id);
            let fire = Intent { missile: true, ..Default::default() };
            w.update(&fire, 1.0 / 60.0);
            assert_eq!(w.missiles.len(), 1);
            assert_eq!(w.missiles[0].target, Some(id), "locked");
            let before = w.ship(id).map(|t| t.hull + t.armour.iter().sum::<f32>()).unwrap();
            fly(&mut w, 4.0);
            w.ship(id).map_or(true, |t| t.hull + t.armour.iter().sum::<f32>() < before)
        };
        assert!(run(0), "no ECM: the seeker strikes");
    }

    #[test]
    fn a_rear_turret_fires_at_what_follows() {
        let mut w = World::new(3);
        w.launch();
        w.encounter_t = f32::INFINITY;
        w.spawn_encounter();
        let behind = w.ships[0].pos + v2(0.0, 500.0);
        w.ships[1].pos = behind;
        w.brains.clear();
        w.ships[0].kit.turret = true;
        let n = w.shots.len();
        w.update(&Intent::default(), 1.0 / 60.0);
        assert!(w.shots.len() > n, "the turret fired");
    }

    #[test]
    fn customs_find_open_contraband_and_hidden_compartments_mostly_hold() {
        // stims in the open, landing at Routa (the cooperatives ban them)
        let mut w = World::new(5);
        w.cargo[econ::STIMS] = 4;
        w.launch();
        w.ships[0].vel = V2::ZERO;
        w.ships[0].pos = w.bases[0].pos + v2(0.0, -400.0);
        w.land();
        assert_eq!(w.cargo[econ::STIMS], 0, "seized");
        assert!(w.credits < START_CREDITS);

        // the same four stims in a small hidden compartment: usually missed
        let mut clean = 0;
        for seed in 0..40 {
            let mut w = World::new(seed);
            w.ships[0].kit.hidden = 5;
            w.cargo[econ::STIMS] = 4;
            w.inspect(Faction::Coop, "Customs".into(), None);
            clean += (w.cargo[econ::STIMS] == 4) as u32;
        }
        assert!(clean > 25, "{clean}/40 got through");
    }

    #[test]
    fn black_markets_deal_in_what_the_law_forbids() {
        let w = World::new(5);
        let all = w.sector.all_bases();
        let at = |name: &str| all.iter().position(|&(r, b)| w.sector.regions[r].bases[b].name == name).unwrap();
        assert!(!w.markets[at("Routa Station")].open[econ::STIMS]);
        assert!(w.markets[at("Vaski Works")].open[econ::STIMS]);
        assert!(w.markets[at("Sisu Clinic")].open[econ::STIMS], "the traders' clinic: legal there");
    }

    #[test]
    fn a_month_of_war_runs_round_the_player() {
        let mut w = World::new(12);
        w.launch();
        for _ in 0..30 {
            w.pass_hours(24.0);
            fly(&mut w, 2.0);
        }
        assert!(w.clock > 24.0 * 30.0);
        assert!(w.war.control.len() == w.sector.regions.len());
    }

    #[test]
    fn a_shot_hits_what_it_passes_through() {
        assert!(seg_hits(v2(0.0, 0.0), v2(100.0, 0.0), v2(50.0, 10.0), 20.0));
        assert!(!seg_hits(v2(0.0, 0.0), v2(100.0, 0.0), v2(150.0, 0.0), 20.0));
    }
}
