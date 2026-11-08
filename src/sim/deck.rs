//! On foot: a base's deck, walked as a classic ASCII roguelike (David,
//! 2026-11-08). One step per key; the crowd moves when you do.
//!
//! The plan of every deck is the same shape, so it always fits 15 rows:
//! the hangar with your ship on the left, then a corridor running east
//! with rooms above and below it. Which rooms, their order and widths, and
//! who is about come from the base and the day.
//!
//! ```text
//! ┌─────────┬──────┬────────┐
//! │         │ = t= │ ...    │   rooms above the corridor
//! │  ship   ├──+───┴──+─────┤
//! │   @     +               │   the corridor
//! │         ├───+──┬────+───┤
//! │    m    │      │        │   rooms below
//! └─────────┴──────┴────────┘
//! ```
//!
//! Walking into someone or something is how you use it (bumping, as in
//! roguelikes): the broker's counter opens the exchange, your ship
//! launches.

use super::body::{Body, ARMS};
use super::faction::Faction;
use super::rng::Rng;
use super::sector::BaseKind;

/// Rows in every deck.
pub const H: i32 = 15;
/// The corridor's row.
pub const MID: i32 = 7;
const HANGAR_W: i32 = 19;
/// `room_of` for tiles in no room (walls, doors)
const NONE: u8 = 255;
/// `room_of` for the corridor
const CORRIDOR: u8 = 254;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Void,
    Wall,
    Floor,
    Door,
    Counter,
    Terminal,
    Crate,
    Table,
    /// the farm's planters
    Plant,
    /// the clinic's beds
    Bed,
    /// the refinery's vats
    Vat,
    /// your ship, parked in the hangar
    Ship,
    /// aboard another ship: the hatch back to your own
    Hatch,
    /// aboard another ship: the cockpit's console
    Console,
}

impl Tile {
    fn open(self) -> bool {
        matches!(self, Tile::Floor | Tile::Door)
    }
}

/// What a room is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Hangar,
    Exchange,
    /// the guilds' office: the mission boards
    Office,
    Bar,
    /// the shipyard or outfitter
    Yard,
    /// the base's own work: an ore hall, a greenhouse, a ward...
    Hall,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub place: Place,
    pub name: &'static str,
    /// walls included
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

/// Who someone is, which is what bumping into them does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Broker,
    Clerk,
    Barkeep,
    Fixer,
    Yardmaster,
    Mechanic,
    Guard,
    /// someone about their business: a pilot, a worker, a spacer
    Patron(u8),
    /// aboard a boarded ship: a raider, a spacer, a patrol officer
    Crew(u8),
    /// someone a quest put here: an index into the deck's `named`
    Named(u8),
}

impl Who {
    pub fn glyph(self) -> char {
        match self {
            Who::Broker => 't',
            Who::Clerk => 'c',
            Who::Barkeep => 'b',
            Who::Fixer => 'f',
            Who::Yardmaster => 'y',
            Who::Mechanic => 'm',
            Who::Guard => 'g',
            Who::Patron(k) => ['p', 'w', 's'][k as usize % 3],
            Who::Crew(k) => ['r', 's', 'g'][k as usize % 3],
            Who::Named(_) => '?',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Who::Broker => "the exchange broker",
            Who::Clerk => "the guild clerk",
            Who::Barkeep => "the bartender",
            Who::Fixer => "a fixer",
            Who::Yardmaster => "the yard master",
            Who::Mechanic => "the dock mechanic",
            Who::Guard => "a guard",
            Who::Patron(k) => ["a pilot", "a worker", "a spacer"][k as usize % 3],
            Who::Crew(k) => ["a raider", "a spacer", "a patrol officer"][k as usize % 3],
            Who::Named(_) => "someone",
        }
    }

    fn wanders(self) -> bool {
        matches!(self, Who::Patron(_))
    }
}

#[derive(Clone, Debug)]
pub struct Npc {
    pub x: i32,
    pub y: i32,
    pub who: Who,
    pub faction: Faction,
    room: u8,
    /// someone who fights: the crew of a boarded ship
    pub fight: Option<Fight>,
}

/// Someone a quest put on the deck, by name.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    /// the quest's id for them
    pub id: String,
    pub name: String,
    pub glyph: char,
}

/// A fighter's state on foot.
#[derive(Clone, Debug)]
pub struct Fight {
    pub hp: i32,
    pub max_hp: i32,
    /// index into `body::ARMS`
    pub arm: u8,
    /// damage taken off every hit
    pub soak: i32,
    /// has seen or heard you
    pub alert: bool,
    pub surrendered: bool,
    /// the share of health below which it may give up; 0 never does
    pub quit_at: f32,
}

impl Npc {
    /// Still a danger: armed, up and not giving up.
    pub fn hostile(&self) -> bool {
        self.fight.as_ref().is_some_and(|f| !f.surrendered && f.hp > 0)
    }
}

/// What a crate or the console holds, aboard a boarded ship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loot {
    /// a good and how many
    pub cargo: Option<(usize, u32)>,
    pub credits: u32,
    /// the deck's `item`, put aboard by a quest
    pub item: bool,
}

/// What walking into something, or Enter beside it, does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// a counter or a terminal: open that room's service
    Service(Place),
    /// the fixer in the bar
    Fixer,
    Repair,
    Launch,
    /// someone to talk to (an index into `npcs`)
    Talk(usize),
    /// someone to fight
    Attack(usize),
    /// a crate or console to empty
    Loot(i32, i32),
    /// back through the hatch to your own ship
    Leave,
}

impl Use {
    /// What it is, for the line under the map.
    pub fn label(self, deck: &Deck) -> String {
        match self {
            Use::Service(Place::Exchange) => "the exchange counter".into(),
            Use::Service(Place::Office) => "the mission boards".into(),
            Use::Service(Place::Yard) => "the yard counter".into(),
            Use::Service(_) => "the counter".into(),
            Use::Fixer => "a fixer, waiting for someone".into(),
            Use::Repair => "the dock mechanic".into(),
            Use::Launch => "your ship".into(),
            Use::Talk(i) if deck.npcs[i].fight.as_ref().is_some_and(|f| f.surrendered) => {
                format!("{}, hands up", deck.name_of(i))
            }
            Use::Talk(i) => deck.name_of(i),
            Use::Attack(i) => format!("{}: walk into them to fight", deck.name_of(i)),
            Use::Loot(x, y) => {
                if deck.tile(x, y) == Tile::Console {
                    "the ship's console".into()
                } else {
                    "a cargo crate".into()
                }
            }
            Use::Leave => "the hatch to your ship".into(),
        }
    }
}

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Moved,
    Blocked,
    Bump(Use),
    /// you fought (a blow or a shot); the turn passed
    Acted,
}

/// What a deck is built from.
#[derive(Clone, Debug)]
pub struct Spec {
    pub kind: BaseKind,
    pub owner: Faction,
    pub black_market: bool,
    /// sells hulls or fits upgrades
    pub yard: bool,
    /// a fixer sits in the bar
    pub fixer: bool,
    /// your ship's footprint in the hangar, in cells
    pub ship: (i32, i32),
}

#[derive(Clone, Debug)]
pub struct Deck {
    pub w: i32,
    pub h: i32,
    tiles: Vec<Tile>,
    room_of: Vec<u8>,
    pub rooms: Vec<Room>,
    pub npcs: Vec<Npc>,
    pub you: (i32, i32),
    /// the top-left of the ship's footprint
    pub ship_at: (i32, i32),
    rng: Rng,
    /// aboard someone else's ship, not walking a base
    pub boarding: bool,
    /// what the crates and the console hold, aboard
    pub loot: Vec<((i32, i32), Loot)>,
    /// what happened, newest last
    pub log: Vec<String>,
    /// people a quest put here (`Who::Named`)
    pub named: Vec<Named>,
    /// aboard: something a quest hid in the hold (the crate whose loot has
    /// `item` set)
    pub item: Option<String>,
}

impl Deck {
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return Tile::Void;
        }
        self.tiles[(y * self.w + x) as usize]
    }

    fn set(&mut self, x: i32, y: i32, t: Tile, room: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            let i = (y * self.w + x) as usize;
            self.tiles[i] = t;
            self.room_of[i] = room;
        }
    }

    fn room_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return NONE;
        }
        self.room_of[(y * self.w + x) as usize]
    }

    /// The room you stand in, if any (the corridor and doorways are none).
    pub fn here(&self) -> Option<&Room> {
        self.rooms.get(self.room_at(self.you.0, self.you.1) as usize)
    }

    /// What to call someone: "the bartender", or a quest's person by name.
    pub fn name_of(&self, i: usize) -> String {
        match self.npcs[i].who {
            Who::Named(k) => self
                .named
                .get(k as usize)
                .map_or("someone".into(), |n| n.name.clone()),
            who => who.name().into(),
        }
    }

    pub fn glyph_of(&self, i: usize) -> char {
        match self.npcs[i].who {
            Who::Named(k) => self.named.get(k as usize).map_or('?', |n| n.glyph),
            who => who.glyph(),
        }
    }

    /// A quest's person, if `i` is one: their id.
    pub fn named_id(&self, i: usize) -> Option<&str> {
        match self.npcs.get(i)?.who {
            Who::Named(k) => self.named.get(k as usize).map(|n| n.id.as_str()),
            _ => None,
        }
    }

    /// Puts a quest's person in the first room of `place` (the corridor if
    /// there is none, or `place` is None), somewhere they block nothing:
    /// not by a door or a counter, and never cutting any floor off.
    /// Returns whether a spot was found.
    pub fn add_named(&mut self, n: Named, faction: Faction, place: Option<Place>) -> bool {
        let room = place
            .and_then(|p| self.rooms.iter().position(|r| r.place == p))
            .map_or(CORRIDOR, |r| r as u8);
        let mut spots = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                if self.room_at(x, y) != room
                    || self.tile(x, y) != Tile::Floor
                    || self.npc_at(x, y).is_some()
                    || self.you == (x, y)
                {
                    continue;
                }
                let crowded = (-1..=1).any(|dx| {
                    (-1..=1).any(|dy| {
                        !matches!(
                            self.tile(x + dx, y + dy),
                            Tile::Floor | Tile::Wall | Tile::Void
                        )
                    })
                });
                if !crowded {
                    spots.push((x, y));
                }
            }
        }
        for i in (1..spots.len()).rev() {
            let j = self.rng.below(i as u32 + 1) as usize;
            spots.swap(i, j);
        }
        let k = self.named.len() as u8;
        let before = self.cut_off().len();
        for (x, y) in spots {
            self.npcs.push(Npc {
                x,
                y,
                who: Who::Named(k),
                faction,
                room,
                fight: None,
            });
            if self.cut_off().len() <= before {
                log::info!("on foot: {} placed at {:?}", n.name, (x, y));
                self.named.push(n);
                return true;
            }
            self.npcs.pop();
        }
        log::warn!("on foot: no room for {}", n.name);
        false
    }

    /// Floor that can't be reached from where you stand, with people who
    /// never move (staff, quests' people) counted as walls.
    pub fn cut_off(&self) -> Vec<(i32, i32)> {
        // crew move, and can be fought: only staff count as walls
        let still = |x, y| {
            self.npcs
                .iter()
                .any(|n| n.x == x && n.y == y && !n.who.wanders() && n.fight.is_none())
        };
        let mut seen = vec![false; (self.w * self.h) as usize];
        let at = |x: i32, y: i32| (y * self.w + x) as usize;
        let mut todo = vec![self.you];
        seen[at(self.you.0, self.you.1)] = true;
        while let Some((x, y)) = todo.pop() {
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let p = (x + dx, y + dy);
                if self.tile(p.0, p.1).open() && !still(p.0, p.1) && !seen[at(p.0, p.1)] {
                    seen[at(p.0, p.1)] = true;
                    todo.push(p);
                }
            }
        }
        let mut out = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                if self.tile(x, y) == Tile::Floor && !still(x, y) && !seen[at(x, y)] {
                    out.push((x, y));
                }
            }
        }
        out
    }

    pub fn npc_at(&self, x: i32, y: i32) -> Option<usize> {
        self.npcs.iter().position(|n| n.x == x && n.y == y)
    }

    /// What touching (x, y) would do.
    fn use_at(&self, x: i32, y: i32) -> Option<Use> {
        if let Some(i) = self.npc_at(x, y) {
            if self.npcs[i].hostile() {
                return Some(Use::Attack(i));
            }
            return Some(match self.npcs[i].who {
                Who::Broker => Use::Service(Place::Exchange),
                Who::Clerk => Use::Service(Place::Office),
                Who::Yardmaster => Use::Service(Place::Yard),
                Who::Fixer => Use::Fixer,
                Who::Mechanic => Use::Repair,
                _ => Use::Talk(i),
            });
        }
        let place = self.rooms.get(self.room_at(x, y) as usize).map(|r| r.place);
        match (self.tile(x, y), place) {
            (Tile::Ship, _) => Some(Use::Launch),
            (Tile::Hatch, _) => Some(Use::Leave),
            (Tile::Crate | Tile::Console, _) if self.loot_at(x, y).is_some() => Some(Use::Loot(x, y)),
            (Tile::Terminal, _) => Some(Use::Service(Place::Office)),
            (Tile::Counter, Some(Place::Bar)) => self.npcs.iter().position(|n| n.who == Who::Barkeep).map(Use::Talk),
            (Tile::Counter, Some(p)) => Some(Use::Service(p)),
            _ => None,
        }
    }

    /// The thing beside you worth using, people first.
    pub fn nearby(&self) -> Option<Use> {
        let (x, y) = self.you;
        let around = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];
        let uses: Vec<Use> = around.iter().filter_map(|(dx, dy)| self.use_at(x + dx, y + dy)).collect();
        uses.iter().find(|u| matches!(u, Use::Attack(_))).or(uses.iter().find(|u| matches!(u, Use::Talk(_) | Use::Fixer))).or(uses.first()).copied()
    }

    /// One step (diagonals allowed). The crowd moves after you do.
    pub fn step(&mut self, dx: i32, dy: i32, body: &mut Body) -> Step {
        let (x, y) = (self.you.0 + dx, self.you.1 + dy);
        if let Some(Use::Attack(i)) = self.use_at(x, y) {
            self.strike(i, body);
            self.turn(body);
            return Step::Acted;
        }
        if let Some(u) = self.use_at(x, y) {
            // someone idling is never in the way: you squeeze past (Enter
            // talks to them)
            if let Use::Talk(i) = u {
                if self.npcs[i].who.wanders() {
                    let me = self.you;
                    self.you = (x, y);
                    self.npcs[i].x = me.0;
                    self.npcs[i].y = me.1;
                    // it wanders wherever it was pushed to
                    self.npcs[i].room = self.room_at(me.0, me.1);
                    self.turn(body);
                    return Step::Moved;
                }
            }
            return Step::Bump(u);
        }
        // no squeezing diagonally between two walls
        let squeeze = dx != 0 && dy != 0 && !self.tile(self.you.0 + dx, self.you.1).open() && !self.tile(self.you.0, self.you.1 + dy).open();
        if !self.tile(x, y).open() || squeeze {
            return Step::Blocked;
        }
        self.you = (x, y);
        self.turn(body);
        Step::Moved
    }

    /// Time passes a step: people wander about their rooms, and anyone
    /// fighting you takes their turn.
    pub fn turn(&mut self, body: &mut Body) {
        self.crew_turn(body);
        for i in 0..self.npcs.len() {
            if !self.npcs[i].who.wanders() || !self.rng.chance(0.35) {
                continue;
            }
            let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][self.rng.below(4) as usize];
            let n = &self.npcs[i];
            let (x, y) = (n.x + dx, n.y + dy);
            let free = self.tile(x, y) == Tile::Floor && self.room_at(x, y) == n.room && self.npc_at(x, y).is_none() && self.you != (x, y);
            if free {
                self.npcs[i].x = x;
                self.npcs[i].y = y;
            }
        }
    }

    /// Lays out a deck. The same spec and seed give the same deck.
    pub fn build(spec: &Spec, seed: u64) -> Deck {
        let mut rng = Rng::new(seed);
        // the rooms off the corridor, shuffled, alternately above and below
        let mut places = vec![Place::Exchange, Place::Office, Place::Bar, Place::Hall];
        if spec.yard {
            places.push(Place::Yard);
        }
        for i in (1..places.len()).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            places.swap(i, j);
        }
        let widths: Vec<i32> = places.iter().map(|p| if *p == Place::Bar { 17 + rng.below(4) as i32 } else { 12 + rng.below(5) as i32 }).collect();
        let mut ends = [HANGAR_W - 1, HANGAR_W - 1];
        let mut spans = Vec::new();
        for (i, (&p, &w)) in places.iter().zip(&widths).enumerate() {
            let side = i % 2;
            let x0 = ends[side];
            ends[side] = x0 + w - 1;
            spans.push((p, side, x0, ends[side]));
        }
        // both sides end at the same wall: the shorter side's last room grows
        let end = ends[0].max(ends[1]);
        for side in 0..2 {
            if let Some(s) = spans.iter_mut().rev().find(|s| s.1 == side) {
                s.3 = end;
            }
        }
        let w = end + 1;
        let mut d = Deck {
            w,
            h: H,
            boarding: false,
            loot: Vec::new(),
            log: Vec::new(),
            named: Vec::new(),
            item: None,
            tiles: vec![Tile::Void; (w * H) as usize],
            room_of: vec![NONE; (w * H) as usize],
            rooms: Vec::new(),
            npcs: Vec::new(),
            you: (0, 0),
            ship_at: (0, 0),
            rng,
        };

        // the hangar
        d.room(Place::Hangar, hangar_name(spec.kind), 0, 0, HANGAR_W - 1, H - 1);
        // the corridor
        for x in HANGAR_W..end {
            d.set(x, MID, Tile::Floor, CORRIDOR);
        }
        d.set(end, MID, Tile::Wall, NONE);
        for &(p, side, x0, x1) in &spans {
            let (y0, y1) = if side == 0 { (0, MID - 1) } else { (MID + 1, H - 1) };
            d.room(p, room_name(p, spec), x0, y0, x1, y1);
        }
        // doors: the hangar's, then one per room onto the corridor
        d.set(HANGAR_W - 1, MID, Tile::Door, NONE);
        for &(_, side, x0, x1) in &spans {
            let x = x0 + (x1 - x0) / 2;
            d.set(x, if side == 0 { MID - 1 } else { MID + 1 }, Tile::Door, NONE);
        }
        d.fit_hangar(spec);
        for r in 1..d.rooms.len() {
            d.furnish(r, spec);
        }
        // a passer-by or two in the corridor
        for _ in 0..1 + d.rng.below(2) {
            let x = HANGAR_W + 2 + d.rng.below((end - HANGAR_W - 3).max(1) as u32) as i32;
            let who = Who::Patron(d.rng.below(3) as u8);
            let f = crowd(spec, &mut d.rng);
            d.person(x, MID, who, f, CORRIDOR);
        }
        d
    }

    fn room(&mut self, place: Place, name: &'static str, x0: i32, y0: i32, x1: i32, y1: i32) {
        let id = self.rooms.len() as u8;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let edge = x == x0 || x == x1 || y == y0 || y == y1;
                if edge {
                    self.set(x, y, Tile::Wall, NONE);
                } else {
                    self.set(x, y, Tile::Floor, id);
                }
            }
        }
        self.rooms.push(Room { place, name, x0, y0, x1, y1 });
    }

    fn put(&mut self, x: i32, y: i32, t: Tile) {
        let r = self.room_at(x, y);
        if self.tile(x, y) == Tile::Floor {
            self.set(x, y, t, r);
        }
    }

    fn person(&mut self, x: i32, y: i32, who: Who, faction: Faction, room: u8) {
        if self.tile(x, y) == Tile::Floor && self.npc_at(x, y).is_none() {
            self.npcs.push(Npc { x, y, who, faction, room, fight: None });
        }
    }

    /// Your ship, the mechanic beside it, and you at its hatch.
    fn fit_hangar(&mut self, spec: &Spec) {
        let (sw, sh) = (spec.ship.0.clamp(1, HANGAR_W - 6), spec.ship.1.clamp(1, H - 6));
        let (sx, sy) = (HANGAR_W / 2 - sw / 2 - 1, MID - sh / 2);
        for y in sy..sy + sh {
            for x in sx..sx + sw {
                self.put(x, y, Tile::Ship);
            }
        }
        self.ship_at = (sx, sy);
        self.you = (sx + sw + 1, MID);
        for (x, y) in [(1, 1), (2, 1), (1, 2), (1, H - 2), (2, H - 2), (HANGAR_W - 2, 1)] {
            self.put(x, y, Tile::Crate);
        }
        self.person(sx + sw / 2, sy + sh + 1, Who::Mechanic, spec.owner, 0);
        // customs, and the crown's own bases, watch who comes aboard
        if spec.kind == BaseKind::Customs || matches!(spec.owner, Faction::Crown | Faction::Inner) {
            self.person(HANGAR_W - 2, MID - 1, Who::Guard, spec.owner, 0);
        }
    }

    fn furnish(&mut self, r: usize, spec: &Spec) {
        let Room { place, x0, y0, x1, y1, .. } = self.rooms[r].clone();
        let id = r as u8;
        let above = y1 < MID;
        // inside: the far row is the one away from the corridor
        let (ix0, ix1) = (x0 + 1, x1 - 1);
        let far = if above { y0 + 1 } else { y1 - 1 };
        let toward = if above { 1 } else { -1 };
        match place {
            Place::Exchange | Place::Yard => {
                // a flap at the end, so the room behind the counter is open
                for x in ix0..ix1 {
                    self.put(x, far + toward, Tile::Counter);
                }
                self.put(ix0, far, Tile::Crate);
                let who = if place == Place::Exchange { Who::Broker } else { Who::Yardmaster };
                // at the far end, so they never cut the strip behind in two
                self.person(ix0 + 1, far, who, spec.owner, id);
                if place == Place::Yard {
                    for x in [ix0, ix0 + 1, ix1] {
                        self.put(x, far + 3 * toward, Tile::Crate);
                    }
                }
            }
            Place::Office => {
                for x in (ix0 + 1..=ix1 - 1).step_by(3) {
                    self.put(x, far, Tile::Terminal);
                }
                self.person(ix0, far + 2 * toward, Who::Clerk, spec.owner, id);
            }
            Place::Bar => {
                let bar_end = ix0 + (ix1 - ix0) * 3 / 5;
                for x in ix0..=bar_end {
                    self.put(x, far + toward, Tile::Counter);
                }
                self.person(ix0, far, Who::Barkeep, spec.owner, id);
                let row = far + 3 * toward;
                for x in [ix0 + 2, ix0 + 7, ix1 - 2] {
                    self.put(x, row, Tile::Table);
                }
                if spec.fixer {
                    self.person(ix1, far, Who::Fixer, if spec.black_market { Faction::Pirates } else { Faction::Traders }, id);
                }
                for _ in 0..2 + self.rng.below(3) {
                    let x = ix0 + self.rng.below((ix1 - ix0 + 1) as u32) as i32;
                    let y = far + toward * (2 + self.rng.below(3) as i32);
                    let who = Who::Patron(self.rng.below(3) as u8);
                    let f = crowd(spec, &mut self.rng);
                    self.person(x, y, who, f, id);
                }
            }
            Place::Hall => {
                let fill = match spec.kind {
                    BaseKind::Farm => Tile::Plant,
                    BaseKind::Clinic => Tile::Bed,
                    BaseKind::Refinery => Tile::Vat,
                    BaseKind::Hub => Tile::Table,
                    _ => Tile::Crate,
                };
                // rows of whatever the base works with, an aisle between
                let rows = if fill == Tile::Table { vec![far + toward] } else { vec![far, far + 2 * toward] };
                for y in rows {
                    for x in (ix0 + 1..ix1).filter(|x| (x - ix0) % 3 != 0) {
                        self.put(x, y, fill);
                    }
                }
                for _ in 0..1 + self.rng.below(3) {
                    let x = ix0 + self.rng.below((ix1 - ix0 + 1) as u32) as i32;
                    let y = far + toward * (1 + 2 * self.rng.below(2) as i32);
                    let f = crowd(spec, &mut self.rng);
                    self.person(x, y, Who::Patron(1), f, id);
                }
            }
            Place::Hangar => {}
        }
    }
}

/// A boarded ship: what it is and what it carries. The world fills it in;
/// the deck lays it out.
#[derive(Clone, Debug)]
pub struct Boarding {
    /// 0 light, 1 medium, 2 heavy: how many rooms and how big a hold
    pub size: u8,
    pub faction: Faction,
    /// 0 raiders, 1 spacers (traders' crew), 2 patrol officers
    pub crew_kind: u8,
    pub crew: u8,
    /// a crate each
    pub cargo: Vec<(usize, u32)>,
    /// in the console
    pub credits: u32,
    /// something a quest put aboard, in a crate of its own
    pub item: Option<String>,
}

/// Rows aboard a boarded ship.
const SHIP_H: i32 = 9;

impl Deck {
    /// Lays out a boarded ship, west to east: the airlock where your ship
    /// is clamped on, the hold, crew quarters on bigger hulls, the cockpit.
    pub fn board(b: &Boarding, seed: u64) -> Deck {
        let mut rng = Rng::new(seed);
        let mut rooms: Vec<(&'static str, i32)> = vec![("airlock", 5), ("hold", 10 + 4 * b.size as i32 + rng.below(3) as i32)];
        if b.size >= 1 {
            rooms.push(("quarters", 8));
        }
        rooms.push(("cockpit", 9));
        let w = rooms.iter().map(|r| r.1 - 1).sum::<i32>() + 1;
        let mut d = Deck {
            w,
            h: SHIP_H,
            boarding: true,
            loot: Vec::new(),
            log: Vec::new(),
            named: Vec::new(),
            item: b.item.clone(),
            tiles: vec![Tile::Void; (w * SHIP_H) as usize],
            room_of: vec![NONE; (w * SHIP_H) as usize],
            rooms: Vec::new(),
            npcs: Vec::new(),
            you: (2, SHIP_H / 2),
            ship_at: (0, 0),
            rng,
        };
        let mut x0 = 0;
        for &(name, rw) in &rooms {
            let place = if name == "hold" { Place::Hall } else { Place::Hangar };
            d.room(place, name, x0, 0, x0 + rw - 1, SHIP_H - 1);
            x0 += rw - 1;
        }
        // a door through each shared wall, at a row of its own
        for r in 1..d.rooms.len() {
            let x = d.rooms[r].x0;
            let y = 2 + d.rng.below(SHIP_H as u32 - 4) as i32;
            d.set(x, y, Tile::Door, NONE);
        }
        d.set(0, SHIP_H / 2, Tile::Hatch, NONE);
        // the hold: a crate per lot of cargo, and empty ones for cover
        let hold = 1;
        let Room { x0: hx0, x1: hx1, .. } = d.rooms[hold].clone();
        let spots: Vec<(i32, i32)> = (hx0 + 2..hx1 - 1).flat_map(|x| [(x, 2), (x, SHIP_H - 3)]).filter(|&(x, _)| (x - hx0) % 3 != 1).collect();
        let mut spots = spots;
        for i in (1..spots.len()).rev() {
            let j = d.rng.below(i as u32 + 1) as usize;
            spots.swap(i, j);
        }
        for (k, &(x, y)) in spots.iter().enumerate() {
            d.put(x, y, Tile::Crate);
            if let Some(&(g, n)) = b.cargo.get(k) {
                d.loot.push((
                    (x, y),
                    Loot {
                        cargo: Some((g, n)),
                        credits: 0,
                        item: false,
                    },
                ));
            } else if k == b.cargo.len() && b.item.is_some() {
                d.loot.push((
                    (x, y),
                    Loot {
                        cargo: None,
                        credits: 0,
                        item: true,
                    },
                ));
            }
            if k >= b.cargo.len() + 3 {
                break;
            }
        }
        // the quarters' bunks
        if b.size >= 1 {
            let q = d.rooms[2].clone();
            for x in q.x0 + 1..q.x1 {
                d.put(x, 1, Tile::Bed);
            }
        }
        // the cockpit's consoles; one holds the ship's cash
        let c = d.rooms.last().cloned().unwrap();
        for y in 2..SHIP_H - 2 {
            d.put(c.x1 - 1, y, Tile::Console);
        }
        d.loot.push((
            (c.x1 - 1, SHIP_H / 2),
            Loot {
                cargo: None,
                credits: b.credits,
                item: false,
            },
        ));
        // the crew: one at the controls, the rest about the ship
        for k in 0..b.crew.max(1) {
            let r = if k == 0 { d.rooms.len() - 1 } else { 1 + d.rng.below(d.rooms.len() as u32 - 1) as usize };
            let rr = d.rooms[r].clone();
            for _ in 0..20 {
                let x = rr.x0 + 1 + d.rng.below((rr.x1 - rr.x0 - 1) as u32) as i32;
                let y = 1 + d.rng.below(SHIP_H as u32 - 2) as i32;
                if d.tile(x, y) == Tile::Floor && d.npc_at(x, y).is_none() {
                    let (hp, arm, soak, quit_at) = match b.crew_kind {
                        // pistols and carbines: a scattergun is a prize you buy
                        0 => (20, d.rng.below(2) as u8, d.rng.below(3) as i32, 0.25),
                        1 => (14, 0, 0, 0.6),
                        _ => (24, 1, 4, 0.0),
                    };
                    let fight = Fight { hp, max_hp: hp, arm, soak, alert: false, surrendered: false, quit_at };
                    d.npcs.push(Npc { x, y, who: Who::Crew(b.crew_kind), faction: b.faction, room: r as u8, fight: Some(fight) });
                    break;
                }
            }
        }
        d.say("You cut through the airlock. The air smells of burnt wiring.".into());
        d
    }

    pub fn loot_at(&self, x: i32, y: i32) -> Option<Loot> {
        self.loot.iter().find(|(p, _)| *p == (x, y)).map(|(_, l)| *l)
    }

    /// Empties a crate or the console.
    pub fn take_loot(&mut self, x: i32, y: i32) -> Option<Loot> {
        let i = self.loot.iter().position(|(p, _)| *p == (x, y))?;
        Some(self.loot.remove(i).1)
    }

    /// Puts back what didn't fit in your hold.
    pub fn leave_loot(&mut self, x: i32, y: i32, l: Loot) {
        if l.cargo.is_some_and(|c| c.1 > 0) || l.credits > 0 || l.item {
            self.loot.push(((x, y), l));
        }
    }

    /// Crew still fighting.
    pub fn crew_left(&self) -> usize {
        self.npcs.iter().filter(|n| n.hostile()).count()
    }

    pub fn say(&mut self, line: String) {
        if self.log.last() == Some(&line) {
            return;
        }
        log::info!("aboard: {line}");
        self.log.push(line);
        if self.log.len() > 40 {
            self.log.remove(0);
        }
    }

    /// Whether a shot from `a` to `b` is clear: nothing solid between.
    pub fn visible(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (mut x, mut y) = a;
        let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
        let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            if (x, y) == b {
                return true;
            }
            if (x, y) != a && !self.tile(x, y).open() {
                return false;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// The nearest crew member you could shoot at.
    pub fn in_sights(&self, body: &Body) -> Option<usize> {
        let range = body.arm().range;
        self.npcs
            .iter()
            .enumerate()
            .filter(|(_, n)| n.hostile() && dist(self.you, (n.x, n.y)) <= range && self.visible(self.you, (n.x, n.y)))
            .min_by_key(|(_, n)| dist(self.you, (n.x, n.y)))
            .map(|(i, _)| i)
    }

    /// F: a shot at the nearest crew member in sight.
    pub fn fire(&mut self, body: &mut Body) -> Step {
        let Some(i) = self.in_sights(body) else {
            self.say("Nobody in your sights.".into());
            return Step::Blocked;
        };
        let arm = *body.arm();
        let d = dist(self.you, (self.npcs[i].x, self.npcs[i].y));
        let name = self.npcs[i].who.name();
        // gunfire wakes the whole ship
        for n in &mut self.npcs {
            if let Some(f) = &mut n.fight {
                f.alert = true;
            }
        }
        if self.rng.chance(arm.aim - 0.03 * d as f32) {
            let (part, k) = location(&mut self.rng);
            let dmg = ((self.rng.below((arm.dmg.1 - arm.dmg.0 + 1) as u32) as i32 + arm.dmg.0) as f32 * k).round() as i32;
            self.say(format!("Your {} hits {name} in the {part}.", arm.name));
            self.wound(i, dmg);
        } else {
            self.say(format!("Your {} misses {name}.", arm.name));
        }
        self.turn(body);
        Step::Acted
    }

    /// A blow at close quarters.
    fn strike(&mut self, i: usize, _body: &mut Body) {
        let name = self.npcs[i].who.name();
        if let Some(f) = &mut self.npcs[i].fight {
            f.alert = true;
        }
        if self.rng.chance(0.85) {
            let dmg = 3 + self.rng.below(5) as i32;
            self.say(format!("You hit {name} hard."));
            self.wound(i, dmg);
        } else {
            self.say(format!("{} ducks your swing.", cap(name)));
        }
    }

    /// M: a medkit, if you have one and need it; it takes a turn.
    pub fn heal(&mut self, body: &mut Body) -> bool {
        if !body.heal() {
            self.say(if body.medkits == 0 { "No medkits left.".into() } else { "You don't need patching up.".into() });
            return false;
        }
        self.say(format!("You patch yourself up ({} medkits left).", body.medkits));
        self.turn(body);
        true
    }

    fn wound(&mut self, i: usize, dmg: i32) {
        let name = self.npcs[i].who.name();
        let quit = self.rng.chance(0.5);
        let Some(f) = &mut self.npcs[i].fight else { return };
        f.hp -= (dmg - f.soak).max(1);
        if f.hp <= 0 {
            self.say(format!("{} goes down.", cap(name)));
        } else if (f.hp as f32) < f.max_hp as f32 * f.quit_at && quit {
            f.surrendered = true;
            self.say(format!("{} throws down their gun. \u{201c}Enough! Take what you want.\u{201d}", cap(name)));
        }
    }

    /// The crew's turn: spot you, shoot when they can, close in when not.
    fn crew_turn(&mut self, body: &mut Body) {
        if !self.boarding {
            return;
        }
        let paths = self.paths_from(self.you);
        for i in 0..self.npcs.len() {
            if !self.npcs[i].hostile() || body.hp <= 0 {
                continue;
            }
            let at = (self.npcs[i].x, self.npcs[i].y);
            let d = dist(at, self.you);
            let seen = self.visible(at, self.you);
            let name = self.npcs[i].who.name();
            let Some(f) = self.npcs[i].fight.clone() else { continue };
            if !f.alert {
                if seen && d <= 9 {
                    self.npcs[i].fight.as_mut().unwrap().alert = true;
                    self.say(format!("{} spots you!", cap(name)));
                }
                continue;
            }
            let arm = ARMS[(f.arm as usize).min(ARMS.len() - 1)];
            if seen && d <= arm.range {
                if self.rng.chance(arm.aim * 0.65 - 0.04 * d as f32) {
                    let (part, k) = location(&mut self.rng);
                    let raw = ((self.rng.below((arm.dmg.1 - arm.dmg.0 + 1) as u32) as i32 + arm.dmg.0) as f32 * k).round() as i32;
                    let dmg = (raw - body.soak()).max(1);
                    body.hp -= dmg;
                    self.say(format!("{} shoots you in the {part} ({dmg}).", cap(name)));
                    if body.hp <= 0 {
                        self.say("You go down.".into());
                    }
                } else {
                    self.say(format!("{}'s shot goes wide.", cap(name)));
                }
                continue;
            }
            // closer, by the shortest way round
            let best = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)]
                .iter()
                .map(|(dx, dy)| (at.0 + dx, at.1 + dy))
                .filter(|&p| self.tile(p.0, p.1).open() && self.npc_at(p.0, p.1).is_none() && p != self.you)
                .filter_map(|p| paths.get(&p).map(|&c| (c, p)))
                .min();
            if let Some((c, p)) = best {
                if c < paths.get(&at).copied().unwrap_or(i32::MAX) {
                    self.npcs[i].x = p.0;
                    self.npcs[i].y = p.1;
                    self.npcs[i].room = self.room_at(p.0, p.1);
                }
            }
        }
        self.npcs.retain(|n| n.fight.as_ref().is_none_or(|f| f.hp > 0));
    }

    /// Steps from `from` to every open tile.
    fn paths_from(&self, from: (i32, i32)) -> std::collections::HashMap<(i32, i32), i32> {
        let mut m = std::collections::HashMap::new();
        m.insert(from, 0);
        let mut q = std::collections::VecDeque::from([from]);
        while let Some(p) = q.pop_front() {
            let c = m[&p];
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)] {
                let n = (p.0 + dx, p.1 + dy);
                if self.tile(n.0, n.1).open() && !m.contains_key(&n) {
                    m.insert(n, c + 1);
                    q.push_back(n);
                }
            }
        }
        m
    }
}

/// Steps between two tiles, diagonals counting one.
fn dist(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Where a shot lands, and what that does to its damage.
fn location(rng: &mut Rng) -> (&'static str, f32) {
    match rng.below(10) {
        0 => ("head", 2.0),
        1 | 2 => (if rng.chance(0.5) { "left arm" } else { "right arm" }, 0.75),
        3 | 4 => (if rng.chance(0.5) { "left leg" } else { "right leg" }, 0.75),
        _ => ("chest", 1.0),
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Who drinks and works here: mostly the owner's people and the Reach's.
fn crowd(spec: &Spec, rng: &mut Rng) -> Faction {
    let mut pool = vec![spec.owner, Faction::Coop, Faction::Traders];
    if spec.black_market {
        pool.push(Faction::Pirates);
    }
    pool[rng.below(pool.len() as u32) as usize]
}

fn hangar_name(kind: BaseKind) -> &'static str {
    match kind {
        BaseKind::Customs => "inspection bay",
        _ => "hangar",
    }
}

fn room_name(p: Place, spec: &Spec) -> &'static str {
    match p {
        Place::Hangar => hangar_name(spec.kind),
        Place::Exchange => "exchange",
        Place::Office => "guild office",
        Place::Bar => "bar",
        Place::Yard if spec.kind.shipyard() => "shipyard",
        Place::Yard if spec.kind == BaseKind::Clinic => "dispensary",
        Place::Yard => "outfitter",
        Place::Hall => match spec.kind {
            BaseKind::Hub => "market hall",
            BaseKind::Mine => "ore hall",
            BaseKind::Refinery => "smelter",
            BaseKind::Farm => "greenhouse",
            BaseKind::Clinic => "ward",
            BaseKind::Customs => "bonded store",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: BaseKind) -> Spec {
        Spec { kind, owner: Faction::Coop, black_market: false, yard: kind.outfitter(), fixer: true, ship: (4, 3) }
    }

    /// Every open tile, reached from where you start.
    fn reach(d: &Deck) -> Vec<(i32, i32)> {
        let mut seen = vec![d.you];
        let mut i = 0;
        while i < seen.len() {
            let (x, y) = seen[i];
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let p = (x + dx, y + dy);
                if d.tile(p.0, p.1).open() && !seen.contains(&p) {
                    seen.push(p);
                }
            }
            i += 1;
        }
        seen
    }

    #[test]
    fn every_kind_of_base_builds_and_every_service_can_be_reached() {
        for kind in [BaseKind::Hub, BaseKind::Mine, BaseKind::Refinery, BaseKind::Farm, BaseKind::Clinic, BaseKind::Customs] {
            for seed in 1..20 {
                let s = spec(kind);
                let d = Deck::build(&s, seed);
                assert_eq!(d.tile(d.you.0, d.you.1), Tile::Floor);
                let open = reach(&d);
                let touch = |u: Use| open.iter().any(|&(x, y)| (-1..=1).any(|dx| (-1..=1).any(|dy| d.use_at(x + dx, y + dy) == Some(u))));
                for u in [Use::Service(Place::Exchange), Use::Service(Place::Office), Use::Fixer, Use::Repair, Use::Launch] {
                    assert!(touch(u), "{kind:?} seed {seed}: can't reach {u:?}");
                }
                assert_eq!(touch(Use::Service(Place::Yard)), s.yard, "{kind:?}: the yard is there when the base has one");
                assert!(d.npcs.iter().any(|n| n.who == Who::Barkeep));
            }
        }
    }

    fn unreachable(d: &Deck) -> Vec<(i32, i32)> {
        d.cut_off()
    }

    #[test]
    fn no_floor_is_walled_off() {
        let mut bad = Vec::new();
        for kind in [BaseKind::Hub, BaseKind::Mine, BaseKind::Refinery, BaseKind::Farm, BaseKind::Clinic, BaseKind::Customs] {
            for seed in 1..200 {
                let d = Deck::build(&spec(kind), seed);
                let u = unreachable(&d);
                if !u.is_empty() {
                    bad.push(format!("{kind:?} seed {seed}: {} tiles, e.g. {:?} in {:?}", u.len(), u[0], d.rooms.get(d.room_at(u[0].0, u[0].1) as usize).map(|r| r.name)));
                }
            }
        }
        for seed in 1..200 {
            for size in 0..3 {
                let d = Deck::board(
                    &Boarding {
                        size,
                        faction: Faction::Pirates,
                        crew_kind: 0,
                        crew: 0,
                        cargo: vec![(0, 5); 4],
                        credits: 9,
                        item: None,
                    },
                    seed,
                );
                let u = unreachable(&d);
                if !u.is_empty() {
                    bad.push(format!("ship size {size} seed {seed}: {} tiles, e.g. {:?}", u.len(), u[0]));
                }
            }
        }
        assert!(bad.is_empty(), "{} decks walled off:\n{}", bad.len(), bad[..bad.len().min(12)].join("\n"));
    }

    #[test]
    fn the_same_base_and_day_give_the_same_deck() {
        let s = spec(BaseKind::Hub);
        let a = Deck::build(&s, 42);
        let b = Deck::build(&s, 42);
        assert_eq!(a.w, b.w);
        assert_eq!(a.tiles, b.tiles);
    }

    #[test]
    fn walking_into_the_ship_launches_and_walls_stop_you() {
        let mut d = Deck::build(&spec(BaseKind::Mine), 3);
        // you start just east of the ship
        assert_eq!(d.step(-1, 0, &mut Body::default()), Step::Moved);
        assert_eq!(d.step(-1, 0, &mut Body::default()), Step::Bump(Use::Launch));
        // the hangar's west wall
        let mut d = Deck::build(&spec(BaseKind::Mine), 3);
        d.you = (1, MID + 3);
        assert_eq!(d.step(-1, 0, &mut Body::default()), Step::Blocked);
    }

    #[test]
    fn walking_into_someone_idle_squeezes_past() {
        let mut d = Deck::build(&spec(BaseKind::Hub), 9);
        d.npcs.retain(|n| !n.who.wanders());
        d.npcs.push(Npc { x: d.you.0 + 1, y: d.you.1, who: Who::Patron(0), faction: Faction::Coop, room: CORRIDOR, fight: None });
        let start = d.you;
        assert_eq!(d.nearby(), Some(Use::Talk(d.npcs.len() - 1)), "Enter would talk");
        assert_eq!(d.step(1, 0, &mut Body::default()), Step::Moved);
        assert_eq!(d.you, (start.0 + 1, start.1));
    }

    #[test]
    fn the_crowd_stays_in_its_rooms() {
        let mut d = Deck::build(&spec(BaseKind::Hub), 9);
        let before: Vec<u8> = d.npcs.iter().map(|n| n.room).collect();
        for _ in 0..500 {
            d.turn(&mut Body::default());
        }
        for (n, r) in d.npcs.iter().zip(before) {
            assert_eq!(d.room_at(n.x, n.y), r, "{:?} wandered off", n.who);
            assert_eq!(d.tile(n.x, n.y), Tile::Floor);
        }
    }

    fn boarding(kind: u8, crew: u8) -> Boarding {
        Boarding {
            size: 1,
            faction: Faction::Pirates,
            crew_kind: kind,
            crew,
            cargo: vec![(0, 6), (10, 3)],
            credits: 300,
            item: None,
        }
    }

    #[test]
    fn a_boarded_ship_has_its_crew_its_loot_and_a_way_home() {
        for seed in 1..30 {
            for size in 0..3 {
                let mut b = boarding(0, 3);
                b.size = size;
                let d = Deck::board(&b, seed);
                assert_eq!(d.crew_left(), 3.min(d.npcs.len()), "seed {seed}");
                assert!(!d.npcs.is_empty());
                assert_eq!(d.loot.len(), 3, "two crates and the console");
                let open = reach(&d);
                for &((x, y), _) in &d.loot {
                    assert!(open.iter().any(|&(ox, oy)| (ox - x).abs() <= 1 && (oy - y).abs() <= 1), "seed {seed} size {size}: loot at {x},{y} out of reach");
                }
                for n in &d.npcs {
                    assert!(open.contains(&(n.x, n.y)), "seed {seed}: crew walled in");
                }
                assert_eq!(d.use_at(0, d.h / 2), Some(Use::Leave));
            }
        }
    }

    #[test]
    fn a_fight_is_won_or_lost_turn_by_turn() {
        // a long fight: you can't miss for ever, and nor can they
        let mut won = 0;
        let mut lost = 0;
        for seed in 1..40 {
            let mut d = Deck::board(&boarding(0, 2), seed);
            let mut me = Body { hp: 30, arm: 1, vest: 1, medkits: 2 };
            for _ in 0..400 {
                if d.crew_left() == 0 || me.hp <= 0 {
                    break;
                }
                if d.fire(&mut me) == Step::Blocked {
                    // nobody in sight: walk toward the nearest
                    let n = d.npcs.iter().find(|n| n.hostile()).unwrap();
                    let paths = d.paths_from((n.x, n.y));
                    let (x, y) = d.you;
                    let next = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)].iter().filter_map(|(dx, dy)| paths.get(&(x + dx, y + dy)).map(|c| (*c, (*dx, *dy)))).min();
                    match next {
                        Some((_, (dx, dy))) => {
                            d.step(dx, dy, &mut me);
                        }
                        None => d.turn(&mut me),
                    }
                }
                if me.hp < 12 {
                    d.heal(&mut me);
                }
            }
            if me.hp <= 0 {
                lost += 1;
            } else if d.crew_left() == 0 {
                won += 1;
            }
        }
        assert!(won > 0 && lost > 0, "a fight can go either way: won {won}, lost {lost}");
    }

    #[test]
    fn spacers_give_up_more_readily_than_raiders() {
        let give_up = |kind| {
            (1..200)
                .filter(|&seed| {
                    let mut d = Deck::board(&boarding(kind, 1), seed);
                    let i = 0;
                    d.wound(i, 9);
                    d.npcs[i].fight.as_ref().unwrap().surrendered
                })
                .count()
        };
        assert!(give_up(1) > 40 && give_up(0) == 0, "spacers {} raiders {}", give_up(1), give_up(0));
    }

    #[test]
    fn loot_comes_out_once() {
        let mut d = Deck::board(&boarding(1, 1), 4);
        let ((x, y), l) = d.loot[0];
        assert_eq!(d.use_at(x, y), Some(Use::Loot(x, y)));
        assert_eq!(d.take_loot(x, y), Some(l));
        assert_eq!(d.take_loot(x, y), None);
        assert_eq!(d.use_at(x, y), None, "an empty crate is just cover");
    }
}

#[cfg(test)]
mod odds {
    use super::*;

    /// A fighter who walks to the nearest crew, fires when they can and
    /// heals when hurt. Returns whether it won.
    pub fn fight(crew: u8, seed: u64, me: &mut Body) -> bool {
        let mut d = Deck::board(
            &Boarding {
                size: 1,
                faction: Faction::Pirates,
                crew_kind: 0,
                crew,
                cargo: vec![(0, 5)],
                credits: 9,
                item: None,
            },
            seed,
        );
        for _ in 0..600 {
            if d.crew_left() == 0 {
                return true;
            }
            if me.hp <= 0 {
                return false;
            }
            if me.hp < 14 && me.medkits > 0 {
                d.heal(me);
                continue;
            }
            if d.fire(me) == Step::Blocked {
                let n = d.npcs.iter().find(|n| n.hostile()).unwrap();
                let paths = d.paths_from((n.x, n.y));
                let (x, y) = d.you;
                let next = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)].iter().filter_map(|(dx, dy)| paths.get(&(x + dx, y + dy)).map(|c| (*c, (*dx, *dy)))).min();
                match next {
                    Some((_, (dx, dy))) => {
                        d.step(dx, dy, me);
                    }
                    None => d.turn(me),
                }
            }
        }
        false
    }

    #[test]
    #[ignore]
    fn odds_aboard() {
        for crew in 1..=3 {
            for (name, kit) in [("start", Body::default()), ("carbine+padded", Body { hp: crate::sim::body::MAX_HP, arm: 1, vest: 1, medkits: 3 })] {
                let won = (1..401).filter(|&s| fight(crew, s, &mut kit.clone())).count();
                println!("{crew} raiders, {name}: won {:.0}%", won as f32 / 4.0);
            }
        }
    }
}
