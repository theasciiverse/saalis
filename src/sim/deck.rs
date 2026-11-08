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
            Use::Talk(i) => deck.npcs[i].who.name().into(),
        }
    }
}

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Moved,
    Blocked,
    Bump(Use),
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
    tiles: Vec<Tile>,
    room_of: Vec<u8>,
    pub rooms: Vec<Room>,
    pub npcs: Vec<Npc>,
    pub you: (i32, i32),
    /// the top-left of the ship's footprint
    pub ship_at: (i32, i32),
    /// the person you last walked into: walk into them again to squeeze past
    bumped: Option<usize>,
    rng: Rng,
}

impl Deck {
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= H {
            return Tile::Void;
        }
        self.tiles[(y * self.w + x) as usize]
    }

    fn set(&mut self, x: i32, y: i32, t: Tile, room: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < H {
            let i = (y * self.w + x) as usize;
            self.tiles[i] = t;
            self.room_of[i] = room;
        }
    }

    fn room_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= H {
            return NONE;
        }
        self.room_of[(y * self.w + x) as usize]
    }

    /// The room you stand in, if any (the corridor and doorways are none).
    pub fn here(&self) -> Option<&Room> {
        self.rooms.get(self.room_at(self.you.0, self.you.1) as usize)
    }

    pub fn npc_at(&self, x: i32, y: i32) -> Option<usize> {
        self.npcs.iter().position(|n| n.x == x && n.y == y)
    }

    /// What touching (x, y) would do.
    fn use_at(&self, x: i32, y: i32) -> Option<Use> {
        if let Some(i) = self.npc_at(x, y) {
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
        uses.iter().find(|u| matches!(u, Use::Talk(_) | Use::Fixer)).or(uses.first()).copied()
    }

    /// One step (diagonals allowed). The crowd moves after you do.
    pub fn step(&mut self, dx: i32, dy: i32) -> Step {
        let (x, y) = (self.you.0 + dx, self.you.1 + dy);
        if let Some(u) = self.use_at(x, y) {
            // the second push past someone idling swaps places with them
            if let Use::Talk(i) = u {
                if self.bumped == Some(i) && self.npcs[i].who.wanders() {
                    let me = self.you;
                    self.you = (x, y);
                    self.npcs[i].x = me.0;
                    self.npcs[i].y = me.1;
                    // it wanders wherever it was pushed to
                    self.npcs[i].room = self.room_at(me.0, me.1);
                    self.bumped = None;
                    self.turn();
                    return Step::Moved;
                }
                self.bumped = Some(i);
            }
            return Step::Bump(u);
        }
        // no squeezing diagonally between two walls
        let squeeze = dx != 0 && dy != 0 && !self.tile(self.you.0 + dx, self.you.1).open() && !self.tile(self.you.0, self.you.1 + dy).open();
        if !self.tile(x, y).open() || squeeze {
            return Step::Blocked;
        }
        self.you = (x, y);
        self.bumped = None;
        self.turn();
        Step::Moved
    }

    /// Time passes a step: people wander about their rooms.
    pub fn turn(&mut self) {
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
        let mut d = Deck { w, tiles: vec![Tile::Void; (w * H) as usize], room_of: vec![NONE; (w * H) as usize], rooms: Vec::new(), npcs: Vec::new(), you: (0, 0), ship_at: (0, 0), bumped: None, rng };

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
            self.npcs.push(Npc { x, y, who, faction, room });
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
        let mid_x = (ix0 + ix1) / 2;
        match place {
            Place::Exchange | Place::Yard => {
                for x in ix0..=ix1 {
                    self.put(x, far + toward, Tile::Counter);
                }
                self.put(ix0, far, Tile::Crate);
                self.put(ix1, far, Tile::Crate);
                let who = if place == Place::Exchange { Who::Broker } else { Who::Yardmaster };
                self.person(mid_x, far, who, spec.owner, id);
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
                self.person(ix0 + 2, far, Who::Barkeep, spec.owner, id);
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
        assert_eq!(d.step(-1, 0), Step::Moved);
        assert_eq!(d.step(-1, 0), Step::Bump(Use::Launch));
        // the hangar's west wall
        let mut d = Deck::build(&spec(BaseKind::Mine), 3);
        d.you = (1, MID + 3);
        assert_eq!(d.step(-1, 0), Step::Blocked);
    }

    #[test]
    fn a_second_push_squeezes_past_someone_in_the_way() {
        let mut d = Deck::build(&spec(BaseKind::Hub), 9);
        d.npcs.retain(|n| !n.who.wanders());
        d.npcs.push(Npc { x: d.you.0 + 1, y: d.you.1, who: Who::Patron(0), faction: Faction::Coop, room: CORRIDOR });
        let start = d.you;
        assert!(matches!(d.step(1, 0), Step::Bump(Use::Talk(_))));
        assert_eq!(d.step(1, 0), Step::Moved);
        assert_eq!(d.you, (start.0 + 1, start.1));
    }

    #[test]
    fn the_crowd_stays_in_its_rooms() {
        let mut d = Deck::build(&spec(BaseKind::Hub), 9);
        let before: Vec<u8> = d.npcs.iter().map(|n| n.room).collect();
        for _ in 0..500 {
            d.turn();
        }
        for (n, r) in d.npcs.iter().zip(before) {
            assert_eq!(d.room_at(n.x, n.y), r, "{:?} wandered off", n.who);
            assert_eq!(d.tile(n.x, n.y), Tile::Floor);
        }
    }
}
