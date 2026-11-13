//! The screens around the simulation: title, flight, the nav map, the base
//! on foot and its counters, and the keys that drive them.

use crate::sim::deck::{Deck, Place, Spec, Step, Use};
use crate::sim::econ;
use crate::sim::quest::{Quests, Tick};
use crate::sim::save;
use crate::sim::{Nav, Phase, World};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Flight,
    Map,
    /// on foot, walking the base's deck
    Base,
    /// at a counter on the base: the exchange, the boards, the bar, the yard
    Dock,
    /// hailing a ship
    Comms,
}

pub struct App {
    pub world: World,
    pub screen: Screen,
    /// the map shows the whole sector, not just this region
    pub map_sector: bool,
    /// the highlighted row: a nav point, or a region on the sector map
    pub map_sel: usize,
    /// the highlighted good at the exchange
    pub dock_row: usize,
    /// the dock's tab: 0 exchange, 1 missions, 2 bar, 3 shipyard
    pub dock_tab: usize,
    /// the highlighted line in the shipyard
    pub yard_row: usize,
    /// the highlighted offer on the mission board
    pub job_row: usize,
    /// a line of news at the dock: what was bought, saved, repaired
    pub dock_note: Option<String>,
    /// what the title offers to continue, if there is a save
    pub saved: Option<String>,
    pub demo: bool,
    /// set when the person asked to quit from a menu
    pub ask_quit: bool,
    /// the ship being hailed
    pub hailing: Option<u32>,
    /// the base's deck, while docked
    pub deck: Option<Deck>,
    /// the stories running in this game
    pub quests: Quests,
    /// frames since the quests last ran
    pub quest_wait: u32,
}

/// The quests run this often (frames), and at once when you talk to
/// someone or answer.
const QUEST_EVERY: u32 = 6;

impl App {
    pub fn new(world: World, screen: Screen, demo: bool) -> App {
        let saved = save::read().map(|s| save::describe(&s));
        App {
            world,
            screen,
            map_sector: false,
            map_sel: 0,
            dock_row: 0,
            dock_tab: 0,
            yard_row: 0,
            job_row: 0,
            dock_note: None,
            saved,
            demo,
            ask_quit: false,
            hailing: None,
            deck: None,
            quests: Quests::new(Vec::new()),
            quest_wait: 0,
        }
    }

    /// Whether you are walking a deck.
    fn on_foot(&self) -> bool {
        self.deck.is_some() && matches!(self.screen, Screen::Base | Screen::Dock)
    }

    /// Runs the quests now and then (every frame is too often to matter),
    /// and puts their people on the deck.
    pub fn tick_quests(&mut self) {
        if self.screen == Screen::Title || self.demo {
            return;
        }
        self.quest_wait += 1;
        if self.quest_wait >= QUEST_EVERY {
            self.quest_wait = 0;
            let t = Tick {
                on_foot: self.on_foot(),
                talked: None,
            };
            self.quests.tick(&mut self.world, &t);
        }
        self.place_people();
    }

    /// The quests' people, on this base's deck, if they changed.
    fn place_people(&mut self) {
        if !std::mem::take(&mut self.quests.people_changed) {
            return;
        }
        let Phase::Docked(b) = self.world.phase else {
            return;
        };
        let Some(deck) = self.deck.as_mut().filter(|d| !d.boarding) else {
            return;
        };
        // those who left go; those who came stand somewhere; the rest stay put
        let want = self.quests.people_at(self.world.bases[b].name);
        let gone: Vec<usize> = (0..deck.npcs.len()).filter(|&i| deck.named_id(i).is_some_and(|id| !want.iter().any(|w| w.0.id == id))).collect();
        for i in gone.into_iter().rev() {
            deck.npcs.remove(i);
        }
        for (n, f, place) in want {
            if !(0..deck.npcs.len()).any(|i| deck.named_id(i) == Some(n.id.as_str())) {
                deck.add_named(n, f, place);
            }
        }
    }

    /// Whether keys go to a menu rather than to flight.
    pub fn in_menu(&self) -> bool {
        // an ending's card takes the keys wherever it shows
        self.world.ending.is_some() || matches!(self.screen, Screen::Map | Screen::Base | Screen::Dock | Screen::Comms)
    }

    pub fn new_game(&mut self, seed: u64) {
        self.world = World::new(seed);
        self.quests = Quests::load_all();
        // the quests' first look at the world, so their people are in place
        let t = Tick {
            on_foot: true,
            talked: None,
        };
        self.quests.tick(&mut self.world, &t);
        self.go_aboard();
        self.dock_note = Some("Welcome to Routa. The Reach is yours to work.".into());
        log::info!("new game seed={seed}");
    }

    /// Loads the save; returns whether there was one.
    pub fn continue_game(&mut self) -> bool {
        let Some(s) = save::read() else { return false };
        let quests = s.quests.clone();
        match World::from_save(s) {
            Some(w) => {
                self.world = w;
                self.quests = Quests::load_all();
                self.quests.restore(&quests);
                self.go_aboard();
                self.dock_note = Some("Saved game loaded.".into());
                log::info!("continue: {}", self.world.region_name());
                true
            }
            None => false,
        }
    }

    pub fn save(&mut self) {
        if let Some(mut s) = self.world.to_save() {
            s.quests = self.quests.save();
            let ok = save::write(&s);
            self.saved = Some(save::describe(&s));
            self.dock_note = Some(if ok { "Game saved.".into() } else { "Could not save: see the session log.".into() });
        }
    }

    /// Called after every tick: the simulation's phase picks the screen.
    pub fn follow(&mut self) {
        if self.screen == Screen::Title {
            return;
        }
        match self.world.phase {
            Phase::Docked(_) if !matches!(self.screen, Screen::Base | Screen::Dock) => {
                self.go_aboard();
                self.dock_note = None;
                // every landing is a save point
                if !self.demo {
                    self.save();
                }
            }
            Phase::Boarding(id) if !self.deck.as_ref().is_some_and(|d| d.boarding) => {
                if let Some(b) = self.world.boarding(id) {
                    let seed = self.world.seed ^ (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ self.world.clock.to_bits();
                    self.deck = Some(Deck::board(&b, seed));
                    self.screen = Screen::Base;
                    self.dock_note = None;
                    log::info!("on foot aboard ship {id}");
                }
            }
            Phase::Flying if matches!(self.screen, Screen::Base | Screen::Dock) => {
                self.screen = Screen::Flight;
                self.deck = None;
            }
            Phase::Lost if matches!(self.screen, Screen::Map | Screen::Comms | Screen::Base) => {
                self.screen = Screen::Flight;
                self.deck = None;
            }
            _ => {}
        }
    }

    /// Steps off the ship onto the base's deck, by the hatch.
    pub fn go_aboard(&mut self) {
        let w = &self.world;
        let Phase::Docked(b) = w.phase else { return };
        let base = &w.bases[b];
        let art = w.ships.first().map_or(&[][..], |p| crate::render::glyphs::schematic(p.class));
        let ship = (art.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32, art.len() as i32);
        let spec = Spec { kind: base.kind, owner: w.owner(), black_market: base.black_market, yard: !w.shop().is_empty(), fixer: w.fixer.is_some(), ship };
        // the same base looks the same all day; the crowd changes daily
        let day = (w.clock / 24.0) as u64;
        let seed = w.seed ^ (b as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ day.wrapping_mul(0xD1B5_4A32_D192_ED03);
        self.deck = Some(Deck::build(&spec, seed));
        self.screen = Screen::Base;
        log::info!("aboard {} on foot", base.name);
        self.quests.people_changed = true;
        self.place_people();
    }

    pub fn open_map(&mut self) {
        if self.world.phase != Phase::Flying {
            return;
        }
        self.screen = Screen::Map;
        self.map_sector = false;
        let navs = self.world.navs();
        self.map_sel = self.world.nav.and_then(|n| navs.iter().position(|&x| x == n)).unwrap_or(0);
    }

    /// C: hail the target, or the nearest ship.
    pub fn open_comms(&mut self) {
        if self.world.phase != Phase::Flying {
            return;
        }
        match self.world.hail_target() {
            Some(id) => {
                self.hailing = Some(id);
                self.screen = Screen::Comms;
            }
            None => self.world.say_note("Comms", "Nobody in range to hail."),
        }
    }

    /// A key on the map, at the dock or on comms.
    pub fn menu_key(&mut self, k: KeyEvent) {
        // an ending's card, once the last lines are read, holds everything
        // until Enter, Space or Esc
        if self.world.ending.is_some() && self.quests.talk.is_empty() {
            if matches!(k.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ')) {
                log::info!("ending card closed");
                self.world.ending = None;
            }
            return;
        }
        match self.screen {
            Screen::Map => self.map_key(k),
            Screen::Base => self.base_key(k),
            Screen::Dock => self.dock_key(k),
            Screen::Comms => self.comms_key(k),
            _ => {}
        }
    }

    /// On foot: arrows, WASD, vi keys (with y u b n for diagonals) or the
    /// numpad walk; walking into something uses it, as does Enter beside it.
    fn base_key(&mut self, k: KeyEvent) {
        let walk = match k.code {
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') | KeyCode::Char('8') => Some((0, -1)),
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') | KeyCode::Char('2') => Some((0, 1)),
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') | KeyCode::Char('4') => Some((-1, 0)),
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') | KeyCode::Char('6') => Some((1, 0)),
            KeyCode::Char('y') | KeyCode::Char('7') | KeyCode::Home => Some((-1, -1)),
            KeyCode::Char('u') | KeyCode::Char('9') | KeyCode::PageUp => Some((1, -1)),
            KeyCode::Char('b') | KeyCode::Char('1') | KeyCode::End => Some((-1, 1)),
            KeyCode::Char('n') | KeyCode::Char('3') | KeyCode::PageDown => Some((1, 1)),
            _ => None,
        };
        // someone is talking to you: read on, or answer
        if let Some(t) = self.quests.talk.front() {
            match k.code {
                KeyCode::Char(c @ '1'..='9') if !t.choices.is_empty() => {
                    if self.quests.answer(c as usize - '0' as usize) {
                        self.run_quests_now(None);
                    }
                }
                KeyCode::Enter
                | KeyCode::Char(' ')
                | KeyCode::Char('e')
                | KeyCode::Char('E')
                | KeyCode::Esc
                    if t.choices.is_empty() =>
                {
                    self.quests.dismiss();
                    self.run_quests_now(None);
                }
                _ => {}
            }
            return;
        }
        let Some(deck) = self.deck.as_mut() else {
            return;
        };
        let body = &mut self.world.body;
        let aboard = deck.boarding;
        if let Some((dx, dy)) = walk {
            match deck.step(dx, dy, body) {
                Step::Bump(u) => self.use_thing(u),
                Step::Moved => self.dock_note = None,
                Step::Blocked | Step::Acted => {}
            }
        } else {
            match k.code {
                KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Char(' ') => match deck.nearby() {
                    Some(u) => self.use_thing(u),
                    None => self.dock_note = Some("Nothing here to use. Walk into someone to talk.".into()),
                },
                KeyCode::Char('.') | KeyCode::Char('5') => deck.turn(body),
                KeyCode::Char('f') | KeyCode::Char('F') if aboard => {
                    deck.fire(body);
                }
                KeyCode::Char('m') | KeyCode::Char('M') if aboard => {
                    deck.heal(body);
                }
                KeyCode::Char('f') | KeyCode::Char('F') => self.save(),
                KeyCode::Esc => self.ask_quit = true,
                _ => {}
            }
        }
        if aboard && self.world.body.hp <= 0 {
            self.world.die_aboard();
        }
    }

    /// Runs the quests at once: after an answer, or talking to `talked`.
    /// Returns whether a quest heard you.
    fn run_quests_now(&mut self, talked: Option<String>) -> bool {
        let t = Tick {
            on_foot: self.on_foot(),
            talked,
        };
        let heard = self.quests.tick(&mut self.world, &t);
        self.place_people();
        heard
    }

    /// Does what touching something on the deck does.
    fn use_thing(&mut self, u: Use) {
        log::info!("on foot: use {u:?}");
        let counter = |app: &mut App, tab: usize| {
            app.dock_tab = tab;
            app.dock_note = None;
            app.screen = Screen::Dock;
        };
        match u {
            Use::Service(Place::Exchange) => counter(self, 0),
            Use::Service(Place::Office) => counter(self, 1),
            Use::Fixer => counter(self, 2),
            Use::Service(Place::Yard) => counter(self, 3),
            Use::Service(_) => {}
            Use::Attack(i) => {
                // Enter beside someone fighting you: a blow at them
                let Some(deck) = self.deck.as_mut() else { return };
                let (dx, dy) = (deck.npcs[i].x - deck.you.0, deck.npcs[i].y - deck.you.1);
                deck.step(dx.signum(), dy.signum(), &mut self.world.body);
            }
            Use::Loot(x, y) => {
                let Some(deck) = self.deck.as_mut() else { return };
                let Some(mut l) = deck.take_loot(x, y) else { return };
                let mut said = Vec::new();
                if l.credits > 0 {
                    self.world.credits += l.credits;
                    said.push(format!("{} cr from the console", l.credits));
                    l.credits = 0;
                }
                if l.item {
                    if let Some(it) = deck.item.take() {
                        said.push(it.clone());
                        log::info!("found {it} aboard");
                        self.world.items.push(it);
                    }
                    l.item = false;
                }
                if let Some((g, n)) = l.cargo {
                    let got = self.world.take_cargo(g, n);
                    let name = econ::GOODS[g].name.to_lowercase();
                    if got > 0 {
                        said.push(format!("{got} {name}"));
                    }
                    if got < n {
                        said.push(format!("{} {name} left: your hold is full", n - got));
                    }
                    l.cargo = Some((g, n - got));
                }
                deck.leave_loot(x, y, l);
                let line = if said.is_empty() { "Nothing worth taking.".to_string() } else { format!("You take {}.", said.join("; ")) };
                deck.say(line);
            }
            Use::Leave => {
                self.world.leave_boarded();
                self.screen = Screen::Flight;
                self.deck = None;
            }
            Use::Launch => {
                self.world.launch();
                self.screen = Screen::Flight;
                self.deck = None;
            }
            Use::Repair => {
                let cost = self.world.repair_cost();
                self.dock_note = Some(if cost == 0 {
                    "\u{201c}She's sound. Nothing for me to do.\u{201d}".into()
                } else if self.world.repair() {
                    format!("The mechanic patches you up: {cost} cr.")
                } else {
                    format!("\u{201c}That's {cost} cr of work. Come back when you have it.\u{201d}")
                });
            }
            Use::Talk(i) if self.deck.as_ref().and_then(|d| d.named_id(i)).is_some() => {
                let id = self
                    .deck
                    .as_ref()
                    .and_then(|d| d.named_id(i))
                    .unwrap_or_default()
                    .to_string();
                if !self.run_quests_now(Some(id.clone())) {
                    let (name, idle) = self.quests.idle_line(&id).unwrap_or_default();
                    let idle = if idle.is_empty() {
                        "They nod, and say nothing.".to_string()
                    } else {
                        format!("\u{201c}{idle}\u{201d}")
                    };
                    self.dock_note = Some(format!("{name}: {idle}"));
                }
            }
            Use::Talk(i) => {
                let Some(n) = self.deck.as_ref().map(|d| d.npcs[i].clone()) else { return };
                use crate::sim::deck::Who;
                let line = match n.who {
                    Who::Crew(_) => "Don't shoot! It's all in the hold. Take it.".into(),
                    Who::Barkeep => self.world.tip(),
                    Who::Guard => "Holds get checked here. Keep yours clean.".into(),
                    _ if n.faction == crate::sim::faction::Faction::Pirates => "Mind your own hold, pilot.".into(),
                    _ => self.world.rumour(),
                };
                let who = n.who.name();
                let mut who = who.to_string();
                if let Some(c) = who.get_mut(0..1) {
                    c.make_ascii_uppercase();
                }
                self.dock_note = Some(format!("{who}: \u{201c}{line}\u{201d}"));
            }
        }
    }

    fn board_key(&mut self, k: KeyEvent) {
        let n = self.world.offers.len();
        let note = match k.code {
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') if n > 0 => {
                self.job_row = (self.job_row + n - 1) % n;
                None
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') if n > 0 => {
                self.job_row = (self.job_row + 1) % n;
                None
            }
            KeyCode::Char('a') | KeyCode::Char('A') => Some(self.world.accept_offer(self.job_row)),
            KeyCode::Char('1') => Some(self.world.join(crate::sim::mission::Guild::Traders)),
            KeyCode::Char('2') => Some(self.world.join(crate::sim::mission::Guild::Watch)),
            _ => None,
        };
        if let Some(r) = note {
            self.dock_note = Some(match r {
                Ok(s) | Err(s) => s,
            });
        }
        self.job_row = self.job_row.min(self.world.offers.len().saturating_sub(1));
    }

    fn yard_key(&mut self, k: KeyEvent) {
        let shop = self.world.shop();
        let n = shop.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') if n > 0 => self.yard_row = (self.yard_row + n - 1) % n,
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') if n > 0 => self.yard_row = (self.yard_row + 1) % n,
            KeyCode::Char('b') | KeyCode::Char('B') | KeyCode::Right => {
                if let Some(o) = shop.get(self.yard_row) {
                    self.dock_note = Some(match self.world.buy_item(o.item) {
                        Ok(s) | Err(s) => s,
                    });
                }
            }
            _ => {}
        }
        self.yard_row = self.yard_row.min(self.world.shop().len().saturating_sub(1));
    }

    fn bar_key(&mut self, k: KeyEvent) {
        if matches!(k.code, KeyCode::Char('a') | KeyCode::Char('A')) {
            self.dock_note = Some(match self.world.accept_fixer() {
                Ok(s) | Err(s) => s,
            });
        }
    }

    fn comms_key(&mut self, k: KeyEvent) {
        let Some(id) = self.hailing else {
            self.screen = Screen::Flight;
            return;
        };
        let n = self.world.hail_lines(id).len();
        if let KeyCode::Char(c @ '1'..='9') = k.code {
            let i = c as usize - '1' as usize;
            if i < n {
                self.world.hail(id, i);
                self.screen = Screen::Flight;
                self.hailing = None;
            }
        } else if matches!(k.code, KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C')) {
            self.screen = Screen::Flight;
            self.hailing = None;
        }
    }

    fn map_key(&mut self, k: KeyEvent) {
        let rows = if self.map_sector { self.world.sector.regions.len() } else { self.world.navs().len() };
        match k.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => self.screen = Screen::Flight,
            KeyCode::Tab => {
                self.map_sector = !self.map_sector;
                self.map_sel = if self.map_sector { self.world.course.last().copied().unwrap_or(self.world.region) } else { 0 };
            }
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') if rows > 0 => {
                self.map_sel = (self.map_sel + rows - 1) % rows;
                // regions off the map are passed over
                while self.map_sector && !self.world.sector.shown(self.map_sel) {
                    self.map_sel = (self.map_sel + rows - 1) % rows;
                }
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') if rows > 0 => {
                self.map_sel = (self.map_sel + 1) % rows;
                while self.map_sector && !self.world.sector.shown(self.map_sel) {
                    self.map_sel = (self.map_sel + 1) % rows;
                }
            }
            KeyCode::Enter | KeyCode::Char('g') | KeyCode::Char('G') => {
                if self.map_sector {
                    if self.map_sel != self.world.region {
                        self.world.plot(self.map_sel);
                    }
                } else if let Some(&n) = self.world.navs().get(self.map_sel) {
                    self.world.nav = Some(n);
                    if let Nav::Mouth(to) = n {
                        if self.world.course.first() != Some(&to) {
                            self.world.course.clear();
                        }
                    }
                }
                self.screen = Screen::Flight;
                if matches!(k.code, KeyCode::Char('g') | KeyCode::Char('G')) {
                    self.world.toggle_autopilot();
                }
            }
            _ => {}
        }
    }

    fn dock_key(&mut self, k: KeyEvent) {
        match k.code {
            // step back from the counter onto the deck
            KeyCode::Esc | KeyCode::Enter => {
                self.screen = Screen::Base;
                self.dock_note = None;
                return;
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {}
            _ if self.dock_tab == 1 => return self.board_key(k),
            _ if self.dock_tab == 2 => return self.bar_key(k),
            _ if self.dock_tab == 3 => return self.yard_key(k),
            _ => {}
        }
        let many = k.modifiers.contains(KeyModifiers::SHIFT);
        let n = |c: char| if many || c.is_ascii_uppercase() { 10 } else { 1 };
        match k.code {
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => self.dock_row = (self.dock_row + econ::N - 1) % econ::N,
            KeyCode::Down | KeyCode::Char('j') => self.dock_row = (self.dock_row + 1) % econ::N,
            KeyCode::Right | KeyCode::Char('b') | KeyCode::Char('B') | KeyCode::Char('+') => {
                let want = if let KeyCode::Char(c) = k.code { n(c) } else if many { 10 } else { 1 };
                let g = self.dock_row;
                let got = self.world.buy(g, want);
                self.dock_note = Some(match got {
                    0 if self.world.cargo_room() == 0 => "The hold is full.".into(),
                    0 if self.world.docked_market().is_some_and(|m| self.world.markets[m].has(g) && self.world.credits < self.world.markets[m].buy_price(g)) => "You can't afford one.".into(),
                    0 => "You can't buy that here now.".into(),
                    _ => format!("Bought {got} {}.", econ::GOODS[g].name.to_lowercase()),
                });
                if got > 0 {
                    self.world.cues.push(crate::sim::cue::Cue::Target);
                }
            }
            KeyCode::Left | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('-') => {
                let want = if let KeyCode::Char(c) = k.code { n(c) } else if many { 10 } else { 1 };
                let g = self.dock_row;
                let sold = self.world.sell(g, want);
                self.dock_note = Some(if sold == 0 { "None of that in the hold.".into() } else { format!("Sold {sold} {}.", econ::GOODS[g].name.to_lowercase()) });
                if sold > 0 {
                    self.world.cues.push(crate::sim::cue::Cue::Target);
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F') => self.save(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyCode;

    fn app(w: World) -> App {
        App {
            world: w,
            screen: Screen::Flight,
            map_sector: false,
            map_sel: 0,
            dock_row: 0,
            dock_tab: 0,
            yard_row: 0,
            job_row: 0,
            dock_note: None,
            saved: None,
            demo: false,
            ask_quit: false,
            hailing: None,
            deck: None,
            quests: Quests::new(Vec::new()),
            quest_wait: 0,
        }
    }

    #[test]
    fn a_boarding_runs_from_the_clamps_to_the_loot_and_back_to_flight() {
        let mut a = app(World::new(4));
        a.world.practice_boarding();
        a.follow();
        assert_eq!(a.screen, Screen::Base);
        assert!(a.deck.as_ref().is_some_and(|d| d.boarding));
        // the crew already dealt with
        let deck = a.deck.as_mut().unwrap();
        deck.npcs.clear();
        let loot: Vec<(i32, i32)> = deck.loot.iter().map(|l| l.0).collect();
        let (credits, held) = (a.world.credits, a.world.cargo_used());
        for (x, y) in loot {
            // stand beside it and press Enter
            let deck = a.deck.as_mut().unwrap();
            deck.you = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].into_iter().find(|&(px, py)| deck.tile(px, py) == crate::sim::deck::Tile::Floor).unwrap();
            a.menu_key(KeyEvent::from(KeyCode::Enter));
        }
        assert!(a.world.credits > credits, "the console's cash");
        assert!(a.world.cargo_used() > held, "the crates' cargo");
        // back to the hatch, and through it
        let deck = a.deck.as_mut().unwrap();
        deck.you = (1, deck.h / 2);
        a.menu_key(KeyEvent::from(KeyCode::Char('h')));
        assert_eq!(a.world.phase, Phase::Flying);
        assert_eq!(a.screen, Screen::Flight);
        assert!(a.deck.is_none());
    }

    /// Reads every line shown, and answers questions with `pick`.
    fn read_all(a: &mut App, pick: char) {
        for _ in 0..20 {
            match a.quests.talk.front() {
                None => return,
                Some(t) if t.choices.is_empty() => a.menu_key(KeyEvent::from(KeyCode::Enter)),
                Some(_) => a.menu_key(KeyEvent::from(KeyCode::Char(pick))),
            }
        }
        panic!("the talking never stopped");
    }

    /// Stands beside the quest's person and presses Enter.
    fn talk_to(a: &mut App, id: &str) {
        let deck = a.deck.as_mut().expect("on foot");
        let i = (0..deck.npcs.len())
            .find(|&i| deck.named_id(i) == Some(id))
            .unwrap_or_else(|| panic!("{id} is on the deck"));
        let (x, y) = (deck.npcs[i].x, deck.npcs[i].y);
        let spot = [
            (x - 1, y),
            (x + 1, y),
            (x, y - 1),
            (x, y + 1),
            (x - 1, y - 1),
            (x + 1, y + 1),
            (x - 1, y + 1),
            (x + 1, y - 1),
        ]
        .into_iter()
        .find(|&(px, py)| {
            let free = deck.tile(px, py) == crate::sim::deck::Tile::Floor && deck.npc_at(px, py).is_none();
            // and nobody else nearer to hand
            free && {
                let was = deck.you;
                deck.you = (px, py);
                let ok = deck.nearby() == Some(Use::Talk(i));
                deck.you = was;
                ok
            }
        })
        .expect("room beside them");
        deck.you = spot;
        a.menu_key(KeyEvent::from(KeyCode::Enter));
    }

    fn frames(a: &mut App, n: u32) {
        for _ in 0..n {
            a.world
                .update(&crate::sim::ship::Intent::default(), 1.0 / 60.0);
            a.follow();
            a.tick_quests();
        }
    }

    #[test]
    fn the_jumped_claim_plays_from_the_bar_to_the_hold_and_back() {
        let mut a = app(World::new(7));
        a.screen = Screen::Base;
        a.quests = Quests::built_in();
        a.quests.tick(
            &mut a.world,
            &Tick {
                on_foot: true,
                talked: None,
            },
        );
        a.go_aboard();
        assert_eq!(
            a.deck.as_ref().unwrap().named.len(),
            1,
            "Ilta waits in Routa's bar"
        );
        assert!(a.quests.journal().any(|q| q.title == "The Jumped Claim"));
        talk_to(&mut a, "ilta");
        assert_eq!(
            a.quests.talk.front().map(|t| t.who.as_str()),
            Some("Ilta Rask")
        );
        read_all(&mut a, '1');
        assert_eq!(a.quests.states[0].stage, "hunt");
        // saved and loaded mid-quest
        let saved = a.quests.save();
        a.quests = Quests::built_in();
        a.quests.restore(&saved);

        // out to Kide Drift; the jumpers turn up after a while
        a.world.launch();
        a.follow();
        a.world.enter_region_for_test(1);
        a.world.busy = false;
        frames(&mut a, 700);
        let carrier = a
            .world
            .ships
            .iter()
            .position(|s| s.tag.as_deref() == Some("carrier"))
            .expect("the Ahma came");
        // the fight, short: the escort and anyone else gone, the Ahma knocked out
        let pirates: Vec<usize> = (0..a.world.ships.len())
            .filter(|&i| i != carrier && a.world.ships[i].id != crate::sim::PLAYER)
            .collect();
        for i in pirates {
            a.world.ships[i].alive = false;
        }
        assert!(
            a.world.knock_out(carrier, crate::sim::PLAYER),
            "a ship carrying a quest's item is always left to board"
        );
        let (pos, id, vel) = (
            a.world.ships[carrier].pos,
            a.world.ships[carrier].id,
            a.world.ships[carrier].vel,
        );
        let p = a.world.player_mut_for_test();
        p.pos = pos + crate::sim::v2::v2(0.0, 120.0);
        p.vel = vel;
        a.world.target = Some(id);
        a.world.land();
        a.follow();
        assert!(
            a.deck
                .as_ref()
                .is_some_and(|d| d.boarding && d.item.is_some())
        );
        // the crew already dealt with; the beacon's crate
        let deck = a.deck.as_mut().unwrap();
        deck.npcs.clear();
        let (x, y) = deck
            .loot
            .iter()
            .find(|l| l.1.item)
            .map(|l| l.0)
            .expect("a crate holds the beacon");
        deck.you = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
            .into_iter()
            .find(|&(px, py)| deck.tile(px, py) == crate::sim::deck::Tile::Floor)
            .unwrap();
        a.menu_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(a.world.items, vec!["Ilta's claim beacon".to_string()]);
        frames(&mut a, 10);
        assert_eq!(a.quests.states[0].stage, "home");
        read_all(&mut a, '1');
        // back through the hatch, home to Routa, and to the bar
        let deck = a.deck.as_mut().unwrap();
        deck.you = (1, deck.h / 2);
        a.menu_key(KeyEvent::from(KeyCode::Char('h')));
        assert_eq!(a.world.phase, Phase::Flying);
        a.world.enter_region_for_test(0);
        a.world.phase = Phase::Docked(0);
        a.follow();
        let credits = a.world.credits;
        talk_to(&mut a, "ilta");
        read_all(&mut a, '1');
        assert_eq!(
            a.quests.states[0].status,
            crate::sim::quest::Status::Done("Ilta's claim is hers again. 1500 cr paid.".into())
        );
        assert_eq!(a.world.credits, credits + 1500);
        assert!(a.world.items.is_empty());
        frames(&mut a, 10);
        let deck = a.deck.as_ref().unwrap();
        assert!((0..deck.npcs.len()).all(|i| deck.named_id(i).is_none()), "she's gone home");
    }

    #[test]
    fn people_stay_put_when_someone_else_leaves() {
        use crate::sim::quest::Def;
        let person = |id: &str, g: &str| format!("title = \"{id}\"\n[people.{id}]\nname = \"{id}\"\nglyph = \"{g}\"\nbase = \"Routa Station\"\nroom = \"bar\"\n");
        let stay = "return { start = function() place('a'); wait(function() return false end) end }";
        let go = "return { start = function() place('b'); wait_talk('b'); finish('') end }";
        let defs = vec![Def::parse("a", &person("a", "a"), stay, "a").map_err(|e| ("a".to_string(), e)), Def::parse("b", &person("b", "b"), go, "b").map_err(|e| ("b".to_string(), e))];
        let mut a = app(World::new(3));
        a.quests = Quests::new(defs);
        a.quests.tick(&mut a.world, &Tick { on_foot: true, talked: None });
        a.go_aboard();
        let deck = a.deck.as_ref().unwrap();
        let spot = |d: &Deck, id: &str| (0..d.npcs.len()).find(|&i| d.named_id(i) == Some(id)).map(|i| (d.npcs[i].x, d.npcs[i].y));
        let before = spot(deck, "a").expect("a is there");
        assert!(spot(deck, "b").is_some());
        talk_to(&mut a, "b");
        let deck = a.deck.as_ref().unwrap();
        assert_eq!(spot(deck, "b"), None, "b left");
        assert_eq!(spot(deck, "a"), Some(before), "a never moved");
    }

    #[test]
    fn a_fight_lost_aboard_ends_the_game() {
        let mut a = app(World::new(4));
        a.world.practice_boarding();
        a.follow();
        a.world.body.hp = 0;
        a.menu_key(KeyEvent::from(KeyCode::Char('.')));
        assert_eq!(a.world.phase, Phase::Lost);
        a.follow();
        assert_eq!(a.screen, Screen::Flight, "the card shows over the field");
    }

    fn dock_at(a: &mut App, base: &str) {
        if matches!(a.world.phase, Phase::Docked(_)) {
            a.world.launch();
            a.follow();
        }
        let (r, _) = a.world.sector.all_bases().into_iter().find(|&(r, b)| a.world.sector.regions[r].bases[b].name == base).expect("the base");
        a.world.enter_region_for_test(r);
        let b = a.world.bases.iter().position(|b| b.name == base).unwrap();
        a.world.phase = Phase::Docked(b);
        a.follow();
        frames(a, 12);
    }

    fn fly_in(a: &mut App, region: &str) {
        if matches!(a.world.phase, Phase::Docked(_)) {
            a.world.launch();
            a.follow();
        }
        let r = a.world.sector.region_named(region).expect("the region");
        a.world.enter_region_for_test(r);
        a.world.encounter_t = f32::INFINITY;
        frames(a, 420);
    }

    fn spawned(a: &App, tag: &str) -> bool {
        a.world.ships.iter().any(|s| s.alive && s.tag.as_deref() == Some(tag))
    }

    fn clear(a: &mut App, tag: &str) {
        assert!(spawned(a, tag), "{tag} came");
        for s in a.world.ships.iter_mut().filter(|s| s.tag.as_deref() == Some(tag)) {
            s.alive = false;
        }
    }

    fn status(a: &App, id: &str) -> crate::sim::quest::Status {
        a.quests.states.iter().find(|q| q.id == id).expect("the quest").status.clone()
    }

    fn stage(a: &App, id: &str) -> String {
        a.quests.states.iter().find(|q| q.id == id).expect("the quest").stage.clone()
    }

    #[test]
    fn the_story_runs_from_the_claim_to_its_end() {
        use crate::sim::quest::{Saved, Status};
        let done = |s: Status| matches!(s, Status::Done(_));
        let mut a = app(World::new(11));
        a.quests = Quests::built_in();
        a.quests.restore(&[Saved { id: "claim".into(), stage: "home".into(), vars: serde_json::Value::Null, objective: None, placed: Vec::new(), status: Status::Done("x".into()) }]);
        a.screen = Screen::Base;
        a.follow();
        frames(&mut a, 12);
        // What Lies Under Kide: word from Ilta after a while
        a.world.clock += 7.0;
        dock_at(&mut a, "Kide Mine");
        talk_to(&mut a, "ilta");
        read_all(&mut a, '1');
        assert_eq!(stage(&a, "under"), "cover");
        fly_in(&mut a, "Kide Drift");
        clear(&mut a, "diggers1");
        frames(&mut a, 300);
        clear(&mut a, "diggers2");
        frames(&mut a, 30);
        assert!(done(status(&a, "under")), "{:?}", status(&a, "under"));
        assert!(a.world.items.iter().any(|i| i == "the Lantern"));
        // Cold Light: Dr. Halme, the courier, the ledger
        dock_at(&mut a, "Sisu Clinic");
        talk_to(&mut a, "aino");
        read_all(&mut a, '1');
        assert_eq!(stage(&a, "cold"), "hunt");
        fly_in(&mut a, "Revontuli Gap");
        assert!(spawned(&a, "courier"));
        a.world.items.push("the jumpers' ledger".into());
        frames(&mut a, 30);
        dock_at(&mut a, "Sisu Clinic");
        talk_to(&mut a, "aino");
        read_all(&mut a, '1');
        assert!(done(status(&a, "cold")), "{:?}", status(&a, "cold"));
        // The Ledger: Korpi's offer, then the key in Kalmisto
        dock_at(&mut a, "Halla Refinery");
        talk_to(&mut a, "korpi");
        read_all(&mut a, '2');
        assert_eq!(a.world.flags.get("korpi_offer").map(String::as_str), Some("true"));
        fly_in(&mut a, "Kalmisto");
        assert!(spawned(&a, "hulk") && spawned(&a, "scavengers"));
        a.world.items.push("the Lantern's key".into());
        frames(&mut a, 30);
        assert!(done(status(&a, "ledger")), "{:?}", status(&a, "ledger"));
        assert_eq!(a.world.flags.get("key").map(String::as_str), Some("true"));
        // The Lantern: Ilta's choice, the fight over the claim, the ending
        dock_at(&mut a, "Routa Station");
        talk_to(&mut a, "ilta");
        read_all(&mut a, '1');
        assert_eq!(a.world.flags.get("choice").map(String::as_str), Some("coop"));
        fly_in(&mut a, "Kide Drift");
        clear(&mut a, "final1");
        frames(&mut a, 300);
        clear(&mut a, "final2");
        frames(&mut a, 30);
        assert!(done(status(&a, "finale")), "{:?}", status(&a, "finale"));
        assert_eq!(a.world.ending.as_ref().map(|e| e.0.as_str()), Some("The Reach's Own Sky"));
        a.menu_key(KeyEvent::from(KeyCode::Enter));
        assert!(a.world.ending.is_none(), "Enter closes the card");
        let dawn = a.world.sector.region_named("Aamunkoi").unwrap();
        assert!(a.world.sector.shown(dawn) && a.world.war.control[dawn] == crate::sim::faction::Faction::Coop);
        assert!(!a.world.items.iter().any(|i| i == "the Lantern"));
        // First Light at the new lane's end
        dock_at(&mut a, "Aamunkoi Light");
        talk_to(&mut a, "ilta");
        read_all(&mut a, '1');
        assert!(done(status(&a, "dawn")), "{:?}", status(&a, "dawn"));
        // the side stories opened on the way, and nothing broke
        for q in &a.quests.states {
            assert!(!matches!(q.status, Status::Broken(_)), "{} broke: {:?}", q.id, q.status);
        }
        assert!(a.quests.journal().any(|q| q.title == "Licence Revoked"));
        assert!(a.quests.journal().any(|q| q.title == "Old Debts"));
    }

    #[test]
    fn every_ending_plays_out() {
        use crate::sim::quest::{Saved, Status};
        for (who, base, region, ending, lane) in [
            ("korpi", "Halla Refinery", "Halla Shoal", "crown", Some("Halla Shoal")),
            ("marr", "Gate Customs", "Crown Gate", "inner", None),
            ("aino", "Sisu Clinic", "Revontuli Gap", "lantern", Some("Revontuli Gap")),
        ] {
            let mut a = app(World::new(12));
            a.quests = Quests::built_in();
            let gone = |id: &str| Saved { id: id.into(), stage: "x".into(), vars: serde_json::Value::Null, objective: None, placed: Vec::new(), status: Status::Done("x".into()) };
            a.quests.restore(&[gone("claim"), gone("under"), gone("cold"), gone("ledger")]);
            a.world.items = vec!["the Lantern".into(), "the Lantern's key".into()];
            a.screen = Screen::Base;
            a.follow();
            frames(&mut a, 12);
            dock_at(&mut a, base);
            talk_to(&mut a, who);
            read_all(&mut a, '1');
            fly_in(&mut a, region);
            clear(&mut a, "final1");
            frames(&mut a, 300);
            clear(&mut a, "final2");
            frames(&mut a, 30);
            assert_eq!(a.world.flags.get("ending").map(String::as_str), Some(ending));
            assert!(a.world.ending.is_some());
            assert!(a.world.items.is_empty());
            let dawn = a.world.sector.region_named("Aamunkoi").unwrap();
            assert_eq!(a.world.sector.shown(dawn), lane.is_some(), "{ending}");
            if let Some(from) = lane {
                assert!(a.world.sector.neighbours(dawn).contains(&a.world.sector.region_named(from).unwrap()));
            }
        }
    }
}
