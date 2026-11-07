//! Keyboard: held flight keys in three tiers, plus one-shot commands.
//!
//! The held-key tiers come from Kaiku's `input.rs` (itself from Entity's):
//!
//! 1. **Exact**: the terminal speaks the kitty keyboard protocol (or the
//!    platform reports key-up, as on Windows), so presses and releases are real.
//! 2. **Timing**: any other terminal sends only presses plus OS key repeat. A
//!    key counts as held while its events keep arriving; the repeat delay and
//!    interval are learned from the events themselves. Before the first repeat
//!    a tap and a hold look the same, so that window is capped.
//! 3. **Latched**: turning stays on until pressed again or reversed; Space
//!    toggles the guns, Tab the afterburner; Up and Down step the throttle.
//!    Selected with `SAALIS_INPUT=latch`.
//!
//! Flight: arrows or W A S D turn and throttle, Space fires, Tab burns.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

const PRE_REPEAT_CAP: Duration = Duration::from_millis(220);
const REPEAT_DELAY_DEFAULT: Duration = Duration::from_millis(500);
const REPEAT_INTERVAL_DEFAULT: Duration = Duration::from_millis(33);
const RELEASE_MIN: Duration = Duration::from_millis(60);
const RELEASE_MAX: Duration = Duration::from_millis(160);
/// Timing mode: the guns keep firing this long after the last Space event. A
/// terminal repeats only the newest key, so holding Space and then turning
/// stops the Space repeats; the linger bridges a quick turn.
const FIRE_LINGER: Duration = Duration::from_millis(450);
/// Latched mode: one press of Up or Down moves the set speed this far.
const LATCH_STEP: f32 = 0.6;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Exact,
    Timing,
    Latched,
}

#[derive(Clone, Copy, Debug)]
struct Repeat {
    delay: Duration,
    interval: Duration,
}

/// One key as seen through presses only (timing mode).
#[derive(Clone, Copy, Default, Debug)]
struct Hold {
    last: Option<Instant>,
    events: u32,
}

impl Hold {
    /// Feeds one press/repeat event; returns true when it is a fresh press.
    fn event(&mut self, now: Instant, rep: &mut Repeat) -> bool {
        if let Some(last) = self.last {
            let gap = now.duration_since(last);
            let tight = gap <= rep.interval * 3;
            let continues = if self.events == 1 {
                gap >= rep.delay.mul_f32(0.5) && gap <= rep.delay.mul_f32(1.6)
            } else {
                gap <= RELEASE_MAX.max(rep.interval * 4)
            };
            if continues {
                self.events += 1;
                let blend = |old: Duration, new: Duration| (old.mul_f32(0.5) + new.mul_f32(0.5)).max(Duration::from_millis(10));
                if self.events == 2 {
                    rep.delay = blend(rep.delay, gap.clamp(Duration::from_millis(100), Duration::from_millis(1000)));
                } else if gap < Duration::from_millis(200) {
                    rep.interval = blend(rep.interval, gap);
                }
                self.last = Some(now);
                return !tight;
            }
        }
        self.last = Some(now);
        self.events = 1;
        true
    }

    fn held(&self, now: Instant, rep: &Repeat) -> bool {
        let Some(last) = self.last else { return false };
        let timeout = if self.events >= 2 { (rep.interval * 5 / 2).clamp(RELEASE_MIN, RELEASE_MAX) } else { PRE_REPEAT_CAP.min(rep.delay + rep.interval * 2) };
        now.duration_since(last) <= timeout
    }

    fn clear(&mut self) {
        *self = Hold::default();
    }
}

/// The held keys of flight.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Fire,
    Burner,
}

impl Key {
    fn opposite(self) -> Option<Key> {
        match self {
            Key::Left => Some(Key::Right),
            Key::Right => Some(Key::Left),
            Key::Up => Some(Key::Down),
            Key::Down => Some(Key::Up),
            _ => None,
        }
    }
}

/// Which key drives a held control: the arrow or its letter twin.
const ARROW: usize = 0;
const LETTER: usize = 1;

/// What flight reads each tick.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Controls {
    /// -1 left .. 1 right: keys give whole steps, a pad's stick anything between
    pub turn: f32,
    /// -1 slower .. 1 faster
    pub throttle: f32,
    pub fire: bool,
    pub burner: bool,
}

/// One-shot commands since the loop last took them.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Commands {
    pub target: bool,
    pub nearest: bool,
    pub full: bool,
    pub stop: bool,
    pub match_speed: bool,
    /// Enter: launch from the title, relaunch after a loss
    pub confirm: bool,
    /// latched mode: throttle steps, signed
    pub step: f32,
    /// N: the nav map
    pub map: bool,
    /// G: autopilot
    pub auto: bool,
    /// J: jump through a lane
    pub jump: bool,
    /// L: land
    pub land: bool,
    /// [ and ]: the previous or next nav point
    pub nav_step: i32,
    /// C: continue the saved game on the title; hail in flight
    pub cont: bool,
}

pub struct Input {
    mode: Mode,
    rep: Repeat,
    down: [[bool; 2]; 6],
    hold: [Hold; 6],
    latch_turn: i8,
    latch_fire: bool,
    latch_burner: bool,
    pub cmd: Commands,
    pub mute: bool,
    pub quit: bool,
    /// Esc: the quit question is up
    pub confirm_quit: bool,
    pub help: bool,
    pub paused: bool,
    pub pad_name: Option<String>,
    /// keys go to a menu (the map, the dock): every press is queued in
    /// `pressed` and nothing flies
    pub menu: bool,
    pub pressed: Vec<KeyEvent>,
}

impl Input {
    pub fn new(kitty: bool) -> Input {
        let mode = if kitty {
            Mode::Exact
        } else if std::env::var("SAALIS_INPUT").is_ok_and(|v| v == "latch") {
            Mode::Latched
        } else {
            Mode::Timing
        };
        Input::with_mode(mode)
    }

    pub fn with_mode(mode: Mode) -> Input {
        Input {
            mode,
            rep: Repeat { delay: REPEAT_DELAY_DEFAULT, interval: REPEAT_INTERVAL_DEFAULT },
            down: [[false; 2]; 6],
            hold: [Hold::default(); 6],
            latch_turn: 0,
            latch_fire: false,
            latch_burner: false,
            cmd: Commands::default(),
            mute: false,
            quit: false,
            confirm_quit: false,
            help: false,
            paused: false,
            pad_name: None,
            menu: false,
            pressed: Vec::new(),
        }
    }

    pub fn mode_note(&self) -> Option<&'static str> {
        match self.mode {
            Mode::Exact => None,
            Mode::Timing => Some("keys: timing"),
            Mode::Latched => Some("keys: latched"),
        }
    }

    /// Forgets every held key (the window lost focus).
    pub fn release_all(&mut self) {
        self.down = [[false; 2]; 6];
        for h in &mut self.hold {
            h.clear();
        }
    }

    /// The flight controls with a pad's folded in: whichever asks for more on
    /// an axis wins, and either can fire or burn.
    pub fn with_pad(c: Controls, pad: &crate::pad::PadState) -> Controls {
        let pick = |key: f32, stick: f32| if stick.abs() > key.abs() { stick } else { key };
        Controls { turn: pick(c.turn, pad.turn), throttle: pick(c.throttle, pad.throttle), fire: c.fire || pad.fire, burner: c.burner || pad.burner }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        let press = k.kind == KeyEventKind::Press;
        let release = k.kind == KeyEventKind::Release;
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if press && (k.code == KeyCode::F(1) || (self.help && k.code == KeyCode::Esc)) {
            self.help = !self.help;
            return;
        }
        if self.help {
            return;
        }
        if self.confirm_quit {
            if press {
                if matches!(k.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                    self.quit = true;
                }
                self.confirm_quit = false;
            }
            return;
        }
        if self.menu {
            if press {
                self.pressed.push(k);
            }
            return;
        }
        if let Some((key, source)) = held_key(&k) {
            self.held(key, source, !release, press, now);
            return;
        }
        if !press {
            return;
        }
        match k.code {
            KeyCode::Char('t') | KeyCode::Char('T') => self.cmd.target = true,
            KeyCode::Char('r') | KeyCode::Char('R') => self.cmd.nearest = true,
            KeyCode::Char('m') | KeyCode::Char('M') => self.cmd.match_speed = true,
            KeyCode::Home => self.cmd.full = true,
            KeyCode::End | KeyCode::Backspace => self.cmd.stop = true,
            KeyCode::Enter => self.cmd.confirm = true,
            KeyCode::Char('n') | KeyCode::Char('N') => self.cmd.map = true,
            KeyCode::Char('g') | KeyCode::Char('G') => self.cmd.auto = true,
            KeyCode::Char('j') | KeyCode::Char('J') => self.cmd.jump = true,
            KeyCode::Char('l') | KeyCode::Char('L') => self.cmd.land = true,
            KeyCode::Char('c') | KeyCode::Char('C') => self.cmd.cont = true,
            KeyCode::Char('[') => self.cmd.nav_step -= 1,
            KeyCode::Char(']') => self.cmd.nav_step += 1,
            KeyCode::Char('p') | KeyCode::Char('P') => self.paused = !self.paused,
            KeyCode::F(8) => self.mute = true,
            KeyCode::Esc => self.confirm_quit = true,
            _ => {}
        }
    }

    fn held(&mut self, key: Key, source: usize, down: bool, press: bool, now: Instant) {
        let i = key as usize;
        match self.mode {
            Mode::Exact => self.down[i][source] = down,
            Mode::Timing => {
                // a fresh press of one direction ends its opposite
                if down && self.hold[i].event(now, &mut self.rep) {
                    if let Some(o) = key.opposite() {
                        self.hold[o as usize].clear();
                    }
                }
            }
            Mode::Latched => {
                if press {
                    match key {
                        Key::Left => self.latch_turn = if self.latch_turn == -1 { 0 } else { -1 },
                        Key::Right => self.latch_turn = if self.latch_turn == 1 { 0 } else { 1 },
                        Key::Up => self.cmd.step += LATCH_STEP,
                        Key::Down => self.cmd.step -= LATCH_STEP,
                        Key::Fire => self.latch_fire = !self.latch_fire,
                        Key::Burner => self.latch_burner = !self.latch_burner,
                    }
                }
            }
        }
    }

    pub fn controls(&self, now: Instant) -> Controls {
        match self.mode {
            Mode::Exact => {
                let on = |k: Key| self.down[k as usize].iter().any(|&b| b);
                Controls {
                    turn: on(Key::Right) as i8 as f32 - on(Key::Left) as i8 as f32,
                    throttle: on(Key::Up) as i8 as f32 - on(Key::Down) as i8 as f32,
                    fire: on(Key::Fire),
                    burner: on(Key::Burner),
                }
            }
            Mode::Timing => {
                let held = |k: Key| self.hold[k as usize].held(now, &self.rep);
                let axis = |neg: Key, pos: Key| -> f32 {
                    match (held(neg), held(pos)) {
                        (true, false) => -1.0,
                        (false, true) => 1.0,
                        (true, true) => {
                            if self.hold[neg as usize].last >= self.hold[pos as usize].last {
                                -1.0
                            } else {
                                1.0
                            }
                        }
                        _ => 0.0,
                    }
                };
                let fire_hold = &self.hold[Key::Fire as usize];
                let fire = held(Key::Fire) || fire_hold.last.is_some_and(|t| now.duration_since(t) <= FIRE_LINGER);
                Controls { turn: axis(Key::Left, Key::Right), throttle: axis(Key::Down, Key::Up), fire, burner: held(Key::Burner) }
            }
            Mode::Latched => Controls { turn: self.latch_turn as f32, throttle: 0.0, fire: self.latch_fire, burner: self.latch_burner },
        }
    }
}

/// The held control a key drives, if any: arrows and W A S D, Space, Tab.
/// Letters count whatever the case, so Shift or Caps Lock changes nothing.
fn held_key(k: &KeyEvent) -> Option<(Key, usize)> {
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match k.code {
        KeyCode::Left => Some((Key::Left, ARROW)),
        KeyCode::Right => Some((Key::Right, ARROW)),
        KeyCode::Up => Some((Key::Up, ARROW)),
        KeyCode::Down => Some((Key::Down, ARROW)),
        KeyCode::Char(' ') => Some((Key::Fire, ARROW)),
        KeyCode::Tab => Some((Key::Burner, ARROW)),
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'a' => Some((Key::Left, LETTER)),
            'd' => Some((Key::Right, LETTER)),
            'w' => Some((Key::Up, LETTER)),
            's' => Some((Key::Down, LETTER)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(base: Instant, t: u64) -> Instant {
        base + Duration::from_millis(t)
    }

    fn key(i: &mut Input, code: KeyCode, kind: KeyEventKind, at: Instant) {
        i.on_key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind), at);
    }

    fn press(i: &mut Input, code: KeyCode, at: Instant) {
        key(i, code, KeyEventKind::Press, at);
    }

    #[test]
    fn arrows_and_wasd_fly_alike() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Exact);
        press(&mut i, KeyCode::Char('a'), t0);
        assert_eq!(i.controls(t0).turn, -1.0);
        key(&mut i, KeyCode::Char('a'), KeyEventKind::Release, t0);
        press(&mut i, KeyCode::Up, t0);
        assert_eq!(i.controls(t0), Controls { turn: 0.0, throttle: 1.0, fire: false, burner: false });
    }

    #[test]
    fn a_tap_is_a_short_nudge() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Timing);
        press(&mut i, KeyCode::Right, t0);
        assert_eq!(i.controls(ms(t0, 100)).turn, 1.0);
        assert_eq!(i.controls(ms(t0, 400)).turn, 0.0, "no repeat arrived: it was a tap");
    }

    #[test]
    fn fire_lingers_through_a_turn() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Timing);
        press(&mut i, KeyCode::Char(' '), t0);
        for k in 0..10 {
            press(&mut i, KeyCode::Char(' '), ms(t0, 500 + 30 * k));
        }
        press(&mut i, KeyCode::Left, ms(t0, 800));
        assert!(i.controls(ms(t0, 1000)).fire);
        assert!(!i.controls(ms(t0, 1400)).fire);
    }

    #[test]
    fn commands_are_one_shots() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Timing);
        press(&mut i, KeyCode::Char('t'), t0);
        press(&mut i, KeyCode::Home, t0);
        assert!(i.cmd.target && i.cmd.full);
        assert_eq!(std::mem::take(&mut i.cmd), Commands { target: true, full: true, ..Default::default() });
    }

    #[test]
    fn latched_mode_steps_the_throttle() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Latched);
        press(&mut i, KeyCode::Up, t0);
        press(&mut i, KeyCode::Up, t0);
        press(&mut i, KeyCode::Left, t0);
        assert!((i.cmd.step - 2.0 * LATCH_STEP).abs() < 1e-6);
        assert_eq!(i.controls(ms(t0, 5000)).turn, -1.0, "turning stays on");
    }

    #[test]
    fn a_menu_takes_every_press_and_nothing_flies() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Exact);
        i.menu = true;
        press(&mut i, KeyCode::Up, t0);
        press(&mut i, KeyCode::Esc, t0);
        assert_eq!(i.pressed.len(), 2);
        assert_eq!(i.controls(t0).throttle, 0.0);
        assert!(!i.confirm_quit, "Esc belongs to the menu");
    }

    #[test]
    fn esc_asks_before_quitting() {
        let t0 = Instant::now();
        let mut i = Input::with_mode(Mode::Timing);
        press(&mut i, KeyCode::Esc, t0);
        assert!(i.confirm_quit && !i.quit);
        press(&mut i, KeyCode::Char('n'), t0);
        assert!(!i.confirm_quit && !i.quit);
        press(&mut i, KeyCode::Esc, t0);
        press(&mut i, KeyCode::Char('y'), t0);
        assert!(i.quit);
    }
}
