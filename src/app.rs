//! The screens around the simulation: title, flight, the nav map, the base
//! on foot and its counters, and the keys that drive them.

use crate::sim::deck::{Deck, Place, Spec, Step, Use};
use crate::sim::econ;
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
}

impl App {
    pub fn new(world: World, screen: Screen, demo: bool) -> App {
        let saved = save::read().map(|s| save::describe(&s));
        App { world, screen, map_sector: false, map_sel: 0, dock_row: 0, dock_tab: 0, yard_row: 0, job_row: 0, dock_note: None, saved, demo, ask_quit: false, hailing: None, deck: None }
    }

    /// Whether keys go to a menu rather than to flight.
    pub fn in_menu(&self) -> bool {
        matches!(self.screen, Screen::Map | Screen::Base | Screen::Dock | Screen::Comms)
    }

    pub fn new_game(&mut self, seed: u64) {
        self.world = World::new(seed);
        self.go_aboard();
        self.dock_note = Some("Welcome to Routa. The Reach is yours to work.".into());
        log::info!("new game seed={seed}");
    }

    /// Loads the save; returns whether there was one.
    pub fn continue_game(&mut self) -> bool {
        match save::read().and_then(World::from_save) {
            Some(w) => {
                self.world = w;
                self.go_aboard();
                self.dock_note = Some("Saved game loaded.".into());
                log::info!("continue: {}", self.world.region_name());
                true
            }
            None => false,
        }
    }

    pub fn save(&mut self) {
        if let Some(s) = self.world.to_save() {
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
        let Some(deck) = self.deck.as_mut() else { return };
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
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') if rows > 0 => self.map_sel = (self.map_sel + rows - 1) % rows,
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') if rows > 0 => self.map_sel = (self.map_sel + 1) % rows,
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
        App { world: w, screen: Screen::Flight, map_sector: false, map_sel: 0, dock_row: 0, dock_tab: 0, yard_row: 0, job_row: 0, dock_note: None, saved: None, demo: false, ask_quit: false, hailing: None, deck: None }
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
}
