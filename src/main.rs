//! Saalis: flight, combat and trade in the Northern Reach of the asteroid
//! belt. 0.1: one belt region round Routa station, and claim-jumpers.

mod audio;
mod input;
mod log;
mod pad;
mod render;
mod sim;

use input::Input;
use ratatui::crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::DefaultTerminal;
use sim::ai::Brain;
use sim::ship::Intent;
use sim::{Phase, World};
use std::io;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_micros(16_667);
const DT: f32 = 1.0 / 60.0;
const SNAPSHOT: Duration = Duration::from_millis(500);

const USAGE: &str = "saalis: flight, combat and trade in the Northern Reach

  saalis            title screen
  saalis --play     straight into the cockpit
  saalis --demo     the game plays itself (any key ends it)
  saalis --seed N   the same region and waves each time
  saalis --mute     no sound
  saalis --version

Session log: ~/.local/state/saalis/session.log (SAALIS_LOG=info for less).
Terminals without key releases: SAALIS_INPUT=latch makes turning and fire toggles.";

struct Args {
    play: bool,
    demo: bool,
    mute: bool,
    seed: Option<u64>,
}

fn args() -> Option<Args> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut out = Args { play: false, demo: false, mute: false, seed: None };
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--version" | "-V" => {
                println!("saalis {}", env!("CARGO_PKG_VERSION"));
                return None;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return None;
            }
            "--play" => out.play = true,
            "--demo" => out.demo = true,
            "--mute" => out.mute = true,
            "--seed" => {
                i += 1;
                out.seed = a.get(i).and_then(|s| s.parse().ok());
            }
            other => {
                eprintln!("saalis: unknown option {other}\n\n{USAGE}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    Some(out)
}

fn main() -> io::Result<()> {
    let Some(args) = args() else { return Ok(()) };
    let log_path = log::init();
    ::log::info!("saalis {} start play={} demo={} seed={:?}", env!("CARGO_PKG_VERSION"), args.play, args.demo, args.seed);
    // sound opens before the screen is taken, so a complaint lands in the shell
    let audio = audio::Audio::new(args.mute);
    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    let _ = execute!(io::stdout(), EnableFocusChange);
    // ratatui's panic hook restores the screen; pop the flags before it runs
    let restore = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if kitty {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = execute!(io::stdout(), DisableFocusChange);
        ::log::error!("panic: {info}");
        restore(info);
    }));
    ::log::info!("kitty_keyboard={kitty}");
    let result = run(&mut terminal, kitty, &args, audio);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(io::stdout(), DisableFocusChange);
    ratatui::restore();
    if let Err(e) = &result {
        eprintln!("saalis: {e}");
        if let Some(p) = log_path {
            eprintln!("session log: {}", p.display());
        }
    }
    result
}

fn fresh_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
}

/// The demo pilot: the AI's brain at a high skill, never running, hunting the
/// nearest pirate; with none about, it loops round the station.
fn autopilot(brain: &mut Brain, w: &mut World) -> Intent {
    let Some(me) = w.player().cloned() else { return Intent::default() };
    brain.target = w.target;
    let obstacles = w.obstacles();
    if brain.target.is_none() {
        let home = w.station.pos;
        let to = home - me.pos;
        let want = if to.len() > 1500.0 { to.heading() } else { to.heading() + 1.6 };
        let turn = (sim::v2::turn_to(me.heading, want) * 3.0).clamp(-1.0, 1.0);
        return Intent { turn, set_speed: Some(180.0), ..Default::default() };
    }
    let ships = w.ships.clone();
    sim::ai::think(brain, &me, &ships, &obstacles, &mut w.rng, DT)
}

fn run(terminal: &mut DefaultTerminal, kitty: bool, args: &Args, mut audio: audio::Audio) -> io::Result<()> {
    let seed = || args.seed.unwrap_or_else(fresh_seed);
    let mut world = World::new(seed());
    let mut title = !args.play && !args.demo;
    let demo = args.demo;
    // behind the title the game flies itself, as in the demo
    let mut pilot = Brain::new(0.85, 0.0);
    let mut input = Input::new(kitty);
    let mut pad = pad::Pad::open();
    let mut aspect = render::view::cell_aspect();
    let mut next = Instant::now() + TICK;
    let mut snap = Instant::now();
    let mut was = (title, Phase::Flying, false, false, false);
    ::log::info!("screen {} aspect={aspect:.2}", if title { "title" } else { "play" });
    while !input.quit {
        pad.poll();
        input.pad_name = pad.name.clone();
        let presses = pad.take_presses();
        if presses.pause {
            input.paused = !input.paused;
        }
        if presses.help {
            input.help = !input.help;
        }
        input.cmd.target |= presses.target;
        input.cmd.nearest |= presses.nearest;
        input.cmd.confirm |= presses.confirm;

        let note = input.mode_note();
        terminal.draw(|f| {
            let ui = render::Ui { world: &world, title, help: input.help, paused: input.paused, confirm_quit: input.confirm_quit, demo, note, aspect };
            render::draw(f, &ui);
        })?;

        if event::poll(next.saturating_duration_since(Instant::now()))? {
            loop {
                match event::read()? {
                    Event::Key(k) => {
                        ::log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                        // the demo plays itself: any key ends it (not a Ctrl
                        // combination such as the ^D of a closed stdin)
                        if demo && k.kind == KeyEventKind::Press && !k.modifiers.contains(KeyModifiers::CONTROL) {
                            input.quit = true;
                        }
                        input.on_key(k, Instant::now());
                    }
                    Event::Resize(w, h) => {
                        aspect = render::view::cell_aspect();
                        ::log::info!("resize {w}x{h} aspect={aspect:.2}");
                    }
                    Event::FocusLost => {
                        input.release_all();
                        if !title && !demo && !input.paused {
                            input.paused = true;
                            ::log::info!("focus lost: paused");
                        }
                    }
                    Event::FocusGained => ::log::debug!("focus gained"),
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        let now = Instant::now();
        let mut steps = 0;
        while now >= next && steps < 5 {
            let cmd = std::mem::take(&mut input.cmd);
            let held = !input.help && !input.confirm_quit && !input.paused;
            if title && cmd.confirm && held {
                title = false;
                world = World::new(seed());
                ::log::info!("launch from the title");
            } else if held {
                if world.phase == Phase::Lost && ((cmd.confirm && world.phase_t > 2.0) || ((title || demo) && world.phase_t > 3.0)) {
                    world.relaunch();
                }
                let intent = if title || demo {
                    autopilot(&mut pilot, &mut world)
                } else {
                    if cmd.target {
                        world.cycle_target();
                    }
                    if cmd.nearest {
                        world.nearest_target();
                    }
                    let c = Input::with_pad(input.controls(now), &pad.state);
                    let mut it = Intent { turn: c.turn, throttle: c.throttle, fire: c.fire, burner: c.burner, set_speed: None };
                    if let Some(p) = world.player() {
                        let max = p.class.max_speed;
                        if cmd.full {
                            it.set_speed = Some(max);
                        } else if cmd.stop {
                            it.set_speed = Some(0.0);
                        } else if cmd.match_speed {
                            it.set_speed = world.match_speed().map(|s| s.min(max));
                        } else if cmd.step != 0.0 {
                            it.set_speed = Some((p.set_speed + cmd.step * max * 0.25).clamp(0.0, max));
                        }
                    }
                    it
                };
                world.update(&intent, DT);
            }
            next += TICK;
            steps += 1;
        }
        if steps == 5 {
            next = now + TICK;
        }

        if std::mem::take(&mut input.mute) {
            audio.muted = !audio.muted;
            ::log::info!("muted: {}", audio.muted);
        }
        for cue in world.cues.drain(..) {
            // the title's dogfight plays quietly: no rumble, no guns
            if title && !matches!(cue, sim::cue::Cue::Kill | sim::cue::Cue::BigKill) {
                continue;
            }
            audio.play(cue);
            if !demo {
                pad.feel(cue);
            }
        }

        let state = (title, world.phase, input.paused, input.help, input.confirm_quit);
        if state != was {
            ::log::info!("state title={} phase={:?} paused={} help={} confirm_quit={}", state.0, state.1, state.2, state.3, state.4);
            was = state;
        }
        if snap.elapsed() >= SNAPSHOT {
            snap = Instant::now();
            ::log::debug!("snapshot {}", world.snapshot());
        }
    }
    ::log::info!("quit wave={} kills={} score={}", world.wave, world.kills, world.score);
    Ok(())
}
