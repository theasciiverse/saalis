//! The screens around the simulation: title, flight, the nav map and the
//! dock, and the keys that drive the map and the dock.

use crate::sim::econ;
use crate::sim::save;
use crate::sim::{Nav, Phase, World};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Flight,
    Map,
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
}

impl App {
    pub fn new(world: World, screen: Screen, demo: bool) -> App {
        let saved = save::read().map(|s| save::describe(&s));
        App { world, screen, map_sector: false, map_sel: 0, dock_row: 0, dock_tab: 0, yard_row: 0, job_row: 0, dock_note: None, saved, demo, ask_quit: false, hailing: None }
    }

    /// Whether keys go to a menu rather than to flight.
    pub fn in_menu(&self) -> bool {
        matches!(self.screen, Screen::Map | Screen::Dock | Screen::Comms)
    }

    pub fn new_game(&mut self, seed: u64) {
        self.world = World::new(seed);
        self.screen = Screen::Dock;
        self.dock_note = Some("Welcome to Routa. The Reach is yours to work.".into());
        log::info!("new game seed={seed}");
    }

    /// Loads the save; returns whether there was one.
    pub fn continue_game(&mut self) -> bool {
        match save::read().and_then(World::from_save) {
            Some(w) => {
                self.world = w;
                self.screen = Screen::Dock;
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
            Phase::Docked(_) if self.screen != Screen::Dock => {
                self.screen = Screen::Dock;
                self.dock_note = None;
                // every landing is a save point
                if !self.demo {
                    self.save();
                }
            }
            Phase::Flying if self.screen == Screen::Dock => self.screen = Screen::Flight,
            Phase::Lost if matches!(self.screen, Screen::Map | Screen::Comms) => self.screen = Screen::Flight,
            _ => {}
        }
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
            Screen::Dock => self.dock_key(k),
            Screen::Comms => self.comms_key(k),
            _ => {}
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
            KeyCode::Tab => {
                self.dock_tab = (self.dock_tab + 1) % 4;
                return;
            }
            KeyCode::BackTab => {
                self.dock_tab = (self.dock_tab + 3) % 4;
                return;
            }
            KeyCode::Char('r') | KeyCode::Char('R') | KeyCode::Char('f') | KeyCode::Char('F') | KeyCode::Enter | KeyCode::Esc => {}
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
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let cost = self.world.repair_cost();
                self.dock_note = Some(if cost == 0 {
                    "Nothing to repair.".into()
                } else if self.world.repair() {
                    "The dock crews patched you up.".into()
                } else {
                    "No repairs possible.".into()
                });
            }
            KeyCode::Char('f') | KeyCode::Char('F') => self.save(),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.world.launch();
                self.screen = Screen::Flight;
            }
            KeyCode::Esc => self.ask_quit = true,
            _ => {}
        }
    }
}
