//! Quests (0.8.0): stories written in Lua 5.4 (David, 2026-11-08), with
//! their cast and lines as TOML. Each quest is a pair of files with the
//! same name: `<id>.toml` (title, people, text) and `<id>.lua` (the logic).
//! The built-in ones are in `data/quests/` and compiled in; players add
//! their own to `quests/` in the save folder. `QUESTS.md` is the guide for
//! writing one.
//!
//! A quest's script returns a table of stages, each a function run as a
//! coroutine: it reads in order and waits with `wait_docked("Halla
//! Refinery")` and the like, and returns the name of the next stage. The
//! game saves which stage each quest is in and its `vars` table; a loaded
//! game starts that stage again from its top. So a stage is a checkpoint.
//!
//! The sandbox (tested below): only the `coroutine`, `table`, `string` and
//! `math` libraries; no `dofile`, `loadfile`, `require` or `string.dump`;
//! `load` takes text only; a memory limit, and a cap on how long a script
//! may run without waiting.

use super::deck::{Named, Place};
use super::faction::{self, Faction};
use super::ship::{self, Side};
use super::{Phase, Tagged, World, econ};
use mlua::{ChunkMode, Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Thread, ThreadStatus, Value, Variadic, VmState};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::rc::Rc;

/// The Lua state's memory, all quests together.
const MEMORY: usize = 32 << 20;
/// The hook runs every this many instructions...
const HOOK_EVERY: u32 = 1000;
/// ...and a script that has run this many hooks without waiting is stopped.
const BUDGET: i64 = 500;
/// Stages one quest may pass through in one tick.
const MAX_STAGES: usize = 16;

/// The built-in quests: id, cast and lines, script.
const BUILT_IN: &[(&str, &str, &str)] = &[("claim", include_str!("../../data/quests/claim.toml"), include_str!("../../data/quests/claim.lua"))];

/// The game's functions, as quests call them (`PRELUDE` builds on them).
const CALLS: &[&str] = &[
    "hours",
    "day",
    "region",
    "docked",
    "flying",
    "aboard",
    "on_foot",
    "credits",
    "room",
    "kills",
    "cargo",
    "standing",
    "has",
    "alive",
    "disabled",
    "boarded",
    "talked",
    "answer_",
    "say",
    "ask_",
    "objective",
    "place",
    "remove",
    "pay",
    "charge",
    "give_cargo",
    "take_cargo",
    "give_item",
    "take_item",
    "shift",
    "spawn",
    "finish_",
    "fail_",
    "random",
];

/// Helpers every quest sees, written in Lua over the game's own functions.
const PRELUDE: &str = r#"
function pause() coroutine.yield() end
function wait(f) while not f() do coroutine.yield() end end
function wait_hours(h) local t = hours() + h; while hours() < t do coroutine.yield() end end
function wait_docked(base) wait(function() return docked() == base end) end
function wait_region(r) wait(function() return region() == r end) end
function wait_flying() wait(flying) end
function wait_talk(who) wait(function() return talked(who) end) end
function wait_boarded(tag) wait(function() return boarded(tag) end) end
function choose(who, prompt, options)
  ask_(who, prompt, options)
  while true do
    local a = answer_()
    if a then return a end
    coroutine.yield()
  end
end
function finish(text) finish_(text or ""); while true do coroutine.yield() end end
function fail(text) fail_(text or ""); while true do coroutine.yield() end end
"#;

/// A quest's cast and lines, from its TOML.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    pub title: String,
    #[serde(default)]
    pub people: BTreeMap<String, Person>,
    #[serde(default)]
    pub text: BTreeMap<String, String>,
}

/// Someone a quest can put on a base's deck.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub name: String,
    /// their letter on the deck
    pub glyph: char,
    /// coop, crown, traders, inner or pirates: the colour they wear
    #[serde(default = "coop")]
    pub faction: String,
    /// the base, by name
    pub base: String,
    /// bar, exchange, boards, yard, hall or hangar; the corridor otherwise
    #[serde(default)]
    pub room: String,
    /// what they say when no stage is waiting to talk to them
    #[serde(default)]
    pub idle: String,
}

fn coop() -> String {
    "coop".into()
}

/// A quest as read from its files.
#[derive(Clone, Debug)]
pub struct Def {
    pub id: String,
    pub meta: Meta,
    script: String,
    /// where it came from, for error messages
    origin: String,
}

impl Def {
    pub fn parse(id: &str, toml_text: &str, script: &str, origin: &str) -> Result<Def, String> {
        let meta: Meta = toml::from_str(toml_text).map_err(|e| format!("{origin}.toml: {e}"))?;
        for (k, p) in &meta.people {
            if faction_named(&p.faction).is_none() {
                return Err(format!("{origin}.toml: {k}: no faction called {}", p.faction));
            }
            if !p.room.is_empty() && place_named(&p.room).is_none() {
                return Err(format!("{origin}.toml: {k}: no room called {}", p.room));
            }
        }
        Ok(Def { id: id.into(), meta, script: script.into(), origin: origin.into() })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Status {
    #[default]
    Running,
    Done(String),
    Failed(String),
    /// the script went wrong: the message is in the session log too
    Broken(String),
}

/// A quest's progress, as saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub id: String,
    pub stage: String,
    #[serde(default)]
    pub vars: serde_json::Value,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub placed: Vec<String>,
    #[serde(default)]
    pub status: Status,
}

/// A quest's progress, live.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub id: String,
    pub title: String,
    pub stage: String,
    pub status: Status,
    /// what to do next, for the journal; none while a quest is unseen
    pub objective: Option<String>,
    /// the people of this quest on the decks, by id
    pub placed: Vec<String>,
    /// the option picked in the last `choose`, 1-based
    answer: Option<usize>,
}

/// A line to show on foot, or a question with options.
#[derive(Clone, Debug, PartialEq)]
pub struct Talk {
    pub quest: usize,
    pub who: String,
    pub text: String,
    pub choices: Vec<String>,
}

struct Handles {
    env: Table,
    stages: Table,
    thread: Option<Thread>,
}

pub struct Quests {
    lua: Lua,
    api: Table,
    defs: Vec<Def>,
    /// None where the quest failed to load
    handles: Vec<Option<Handles>>,
    pub states: Vec<State>,
    /// lines and questions waiting to be shown on foot, oldest first
    pub talk: VecDeque<Talk>,
    /// someone was placed or removed since the deck was last laid out
    pub people_changed: bool,
    /// saved progress of quests whose files are gone, kept for their return
    orphans: Vec<Saved>,
    budget: Rc<Cell<i64>>,
    /// the game's functions for this tick, behind the lasting wrappers in
    /// `api` (so a quest may keep one, as in `wait(flying)`)
    calls: Table,
}

/// What happened since the last tick that quests may wait on.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    /// you are walking a deck (a base or a boarded ship)
    pub on_foot: bool,
    /// you just talked to this person (a quest's person id)
    pub talked: Option<String>,
}

impl Quests {
    /// The built-in quests and the player's own.
    pub fn load_all() -> Quests {
        let mut defs = built_in();
        if let Some(dir) = super::save::dir().map(|d| d.join("quests")) {
            defs.extend(read_dir(&dir));
        }
        Quests::new(defs)
    }

    /// Only the quests that come with the game.
    #[cfg(test)]
    pub fn built_in() -> Quests {
        Quests::new(built_in())
    }

    pub fn new(defs: Vec<Result<Def, (String, String)>>) -> Quests {
        let budget = Rc::new(Cell::new(BUDGET));
        let (lua, api, calls) = sandbox().expect("the Lua sandbox opens");
        let mut q = Quests { lua, api, calls, defs: Vec::new(), handles: Vec::new(), states: Vec::new(), talk: VecDeque::new(), people_changed: false, orphans: Vec::new(), budget };
        for d in defs {
            match d {
                Ok(def) => {
                    if q.defs.iter().any(|x| x.id == def.id) {
                        log::warn!("quest {}: a quest of that name is already loaded; {} skipped", def.id, def.origin);
                        continue;
                    }
                    let st = State { id: def.id.clone(), title: def.meta.title.clone(), stage: "start".into(), status: Status::Running, objective: None, placed: Vec::new(), answer: None };
                    let h = q.open(&def);
                    let st = match h {
                        Ok(_) => st,
                        Err(ref e) => {
                            log::error!("quest {}: {e}", def.id);
                            State { status: Status::Broken(e.clone()), ..st }
                        }
                    };
                    q.handles.push(h.ok());
                    q.states.push(st);
                    q.defs.push(def);
                }
                Err((id, e)) => {
                    log::error!("quest {id}: {e}");
                    q.handles.push(None);
                    q.states.push(State { id: id.clone(), title: id.clone(), stage: "start".into(), status: Status::Broken(e), objective: None, placed: Vec::new(), answer: None });
                    q.defs.push(Def { id, meta: Meta::default(), script: String::new(), origin: String::new() });
                }
            }
        }
        if !q.states.is_empty() {
            log::info!("quests: {}", q.states.iter().map(|s| format!("{} ({:?})", s.id, s.status)).collect::<Vec<_>>().join(", "));
        }
        q
    }

    /// A quest's globals: its own, over the shared ones (out of reach),
    /// with its own copies of the libraries, so changing one changes
    /// only its own.
    fn env(&self) -> mlua::Result<Table> {
        let lua = &self.lua;
        let env = lua.create_table()?;
        let meta = lua.create_table()?;
        meta.set("__index", self.api.clone())?;
        meta.set("__metatable", false)?;
        env.set_metatable(Some(meta))?;
        for lib in ["string", "math", "table", "coroutine"] {
            let mine = lua.create_table()?;
            for pair in self.api.get::<Table>(lib)?.pairs::<Value, Value>() {
                let (k, v) = pair?;
                mine.raw_set(k, v)?;
            }
            env.raw_set(lib, mine)?;
        }
        Ok(env)
    }

    /// Runs a quest's file: its own globals over the shared ones, its
    /// `text` and `vars`, and the table of stages it returns.
    fn open(&self, def: &Def) -> Result<Handles, String> {
        let lua = &self.lua;
        let go = || -> mlua::Result<Handles> {
            let env = self.env()?;
            let text = lua.create_table()?;
            for (k, v) in &def.meta.text {
                text.set(k.as_str(), v.as_str())?;
            }
            env.set("text", text)?;
            env.set("vars", lua.create_table()?)?;
            self.budget.set(BUDGET);
            let b = self.budget.clone();
            lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_EVERY), move |_, _| spend(&b))?;
            let r = lua.load(&def.script).set_name(format!("@{}.lua", def.origin)).set_mode(ChunkMode::Text).set_environment(env.clone()).eval::<Table>();
            lua.remove_hook();
            let stages = r?;
            if !matches!(stages.get::<Value>("start")?, Value::Function(_)) {
                return Err(mlua::Error::runtime("the script must return a table of stages with a `start` function"));
            }
            Ok(Handles { env, stages, thread: None })
        };
        go().map_err(|e| tidy(&e))
    }

    /// The quests to show in the journal: begun, and not broken unless
    /// they're the player's own (whose authors need to see it).
    pub fn journal(&self) -> impl Iterator<Item = &State> {
        self.states.iter().filter(|s| match &s.status {
            Status::Running => s.objective.is_some(),
            Status::Broken(_) => true,
            _ => true,
        })
    }

    /// The people quests have put on the deck of `base`.
    pub fn people_at(&self, base: &str) -> Vec<(Named, Faction, Option<Place>)> {
        let mut out = Vec::new();
        for (i, st) in self.states.iter().enumerate() {
            if st.status != Status::Running {
                continue;
            }
            for id in &st.placed {
                if let Some(p) = self.defs[i].meta.people.get(id).filter(|p| p.base == base) {
                    let f = faction_named(&p.faction).unwrap_or(Faction::Coop);
                    out.push((Named { id: id.clone(), name: p.name.clone(), glyph: p.glyph }, f, place_named(&p.room).flatten()));
                }
            }
        }
        out
    }

    /// What a quest's person says when no stage is listening.
    pub fn idle_line(&self, id: &str) -> Option<(String, String)> {
        self.defs.iter().zip(&self.states).filter(|(_, s)| s.placed.iter().any(|p| p == id)).find_map(|(d, _)| d.meta.people.get(id).map(|p| (p.name.clone(), p.idle.clone())))
    }

    /// The player picked option `k` (1-based) of the question shown first.
    pub fn answer(&mut self, k: usize) -> bool {
        let Some(t) = self.talk.front() else {
            return false;
        };
        if k == 0 || k > t.choices.len() {
            return false;
        }
        let q = t.quest;
        self.talk.pop_front();
        self.states[q].answer = Some(k);
        log::info!("quest {}: answered {k}", self.states[q].id);
        true
    }

    /// The line shown first was read.
    pub fn dismiss(&mut self) {
        if self.talk.front().is_some_and(|t| t.choices.is_empty()) {
            self.talk.pop_front();
        }
    }

    /// Progress to save.
    pub fn save(&self) -> Vec<Saved> {
        let mut out: Vec<Saved> = self
            .states
            .iter()
            .zip(&self.handles)
            // a quest that never loaded keeps its saved entry (in `orphans`)
            .filter(|(_, h)| h.is_some())
            .map(|(s, h)| Saved {
                id: s.id.clone(),
                stage: s.stage.clone(),
                vars: h.as_ref().and_then(|h| h.env.get::<Value>("vars").ok()).map_or(serde_json::Value::Null, |v| to_json(&v, 0)),
                objective: s.objective.clone(),
                placed: s.placed.clone(),
                // one that broke as it ran is saved where it was, to go on
                // from there once it's mended
                status: if matches!(s.status, Status::Broken(_)) { Status::Running } else { s.status.clone() },
            })
            .collect();
        out.extend(self.orphans.iter().cloned());
        out
    }

    /// Takes up saved progress: each quest starts its saved stage again.
    pub fn restore(&mut self, saved: &[Saved]) {
        self.orphans.clear();
        for sv in saved {
            let Some(i) = self.states.iter().position(|s| s.id == sv.id) else {
                log::warn!("quest {}: saved, but its files are gone; kept for when they're back", sv.id);
                self.orphans.push(sv.clone());
                continue;
            };
            if self.handles[i].is_none() {
                // it didn't load: keep its progress for when it's mended
                self.orphans.push(sv.clone());
                continue;
            }
            let st = &mut self.states[i];
            st.stage = sv.stage.clone();
            st.status = sv.status.clone();
            st.objective = sv.objective.clone();
            st.placed = sv.placed.clone();
            st.answer = None;
            if let Some(h) = self.handles[i].as_mut() {
                h.thread = None;
                if let Ok(v) = from_json(&self.lua, &sv.vars, 0) {
                    let v = if matches!(v, Value::Table(_)) { v } else { Value::Table(self.lua.create_table().expect("a table")) };
                    let _ = h.env.set("vars", v);
                }
            }
        }
        self.talk.clear();
        self.people_changed = true;
    }

    /// Runs every quest until it waits. Returns whether a quest heard
    /// `t.talked` (if not, the person says their idle line).
    pub fn tick(&mut self, w: &mut World, t: &Tick) -> bool {
        let Quests { lua, calls, defs, handles, states, talk, people_changed, budget, .. } = self;
        let world = RefCell::new(w);
        let sts = RefCell::new(std::mem::take(states));
        let queue = RefCell::new(std::mem::take(talk));
        let cur = Cell::new(0usize);
        let heard = Cell::new(false);
        let moved = Cell::new(false);
        let defs: &Vec<Def> = defs;
        let name_of = |who: &str| defs[cur.get()].meta.people.get(who).map_or(who.to_string(), |p| p.name.clone());
        let end = |status: Status, text: String| {
            let mut sts = sts.borrow_mut();
            let st = &mut sts[cur.get()];
            log::info!("quest {}: {status:?}", st.id);
            st.status = status;
            st.objective = None;
            if !st.placed.is_empty() {
                st.placed.clear();
                moved.set(true);
            }
            if !text.is_empty() {
                if t.on_foot {
                    queue.borrow_mut().push_back(Talk { quest: cur.get(), who: st.title.clone(), text, choices: Vec::new() });
                } else {
                    world.borrow_mut().say_note(&st.title, &text);
                }
            }
        };
        let r = lua.scope(|s| {
            let set = |name: &str, f: Function| calls.set(name, f);
            // --- what quests can see
            set("hours", s.create_function(|_, ()| Ok(world.borrow().clock))?)?;
            set("day", s.create_function(|_, ()| Ok((world.borrow().clock / 24.0) as i64 + 1))?)?;
            set("region", s.create_function(|_, ()| Ok(world.borrow().region_name().to_string()))?)?;
            set(
                "docked",
                s.create_function(|_, ()| {
                    let w = world.borrow();
                    Ok(match w.phase {
                        Phase::Docked(b) => Some(w.bases[b].name.to_string()),
                        _ => None,
                    })
                })?,
            )?;
            set("flying", s.create_function(|_, ()| Ok(world.borrow().phase == Phase::Flying))?)?;
            set("aboard", s.create_function(|_, ()| Ok(matches!(world.borrow().phase, Phase::Boarding(_))))?)?;
            set("on_foot", s.create_function(|_, ()| Ok(t.on_foot))?)?;
            set("credits", s.create_function(|_, ()| Ok(world.borrow().credits))?)?;
            set("room", s.create_function(|_, ()| Ok(world.borrow().cargo_room()))?)?;
            set("kills", s.create_function(|_, ()| Ok(world.borrow().kills))?)?;
            set("cargo", s.create_function(|_, g: String| Ok(world.borrow().cargo[good(&g)?]))?)?;
            set("standing", s.create_function(|_, f: String| Ok(world.borrow().standing.of(faction(&f)?)))?)?;
            set("has", s.create_function(|_, item: String| Ok(world.borrow().items.iter().filter(|i| **i == item).count()))?)?;
            set("alive", s.create_function(|_, tag: String| Ok(world.borrow().tagged(&tag).0))?)?;
            set("disabled", s.create_function(|_, tag: String| Ok(world.borrow().tagged(&tag).1))?)?;
            set(
                "boarded",
                s.create_function(|_, tag: String| {
                    let w = world.borrow();
                    Ok(match w.phase {
                        Phase::Boarding(id) => w.ship(id).is_some_and(|s| s.tag.as_deref() == Some(tag.as_str())),
                        _ => false,
                    })
                })?,
            )?;
            set(
                "talked",
                s.create_function(|_, who: String| {
                    let mine = sts.borrow()[cur.get()].placed.contains(&who);
                    let yes = mine && t.talked.as_deref() == Some(who.as_str());
                    if yes {
                        heard.set(true);
                    }
                    Ok(yes)
                })?,
            )?;
            set("answer_", s.create_function(|_, ()| Ok(sts.borrow_mut()[cur.get()].answer.take()))?)?;
            // --- what quests can do
            set(
                "say",
                s.create_function(|_, (from, text): (String, String)| {
                    if t.on_foot {
                        queue.borrow_mut().push_back(Talk { quest: cur.get(), who: name_of(&from), text, choices: Vec::new() });
                    } else {
                        world.borrow_mut().say_note(&name_of(&from), &text);
                    }
                    Ok(())
                })?,
            )?;
            set(
                "ask_",
                s.create_function(|_, (who, text, choices): (String, String, Vec<String>)| {
                    if choices.is_empty() || choices.len() > 9 {
                        return Err(mlua::Error::runtime("choose needs one to nine options"));
                    }
                    sts.borrow_mut()[cur.get()].answer = None;
                    queue.borrow_mut().push_back(Talk { quest: cur.get(), who: name_of(&who), text, choices });
                    Ok(())
                })?,
            )?;
            set(
                "objective",
                s.create_function(|_, text: Option<String>| {
                    let st = &mut sts.borrow_mut()[cur.get()];
                    log::info!("quest {}: objective {text:?}", st.id);
                    st.objective = text;
                    Ok(())
                })?,
            )?;
            set(
                "place",
                s.create_function(|_, who: String| {
                    if !defs[cur.get()].meta.people.contains_key(&who) {
                        return Err(mlua::Error::runtime(format!("no person called {who} in this quest's toml")));
                    }
                    let st = &mut sts.borrow_mut()[cur.get()];
                    if !st.placed.contains(&who) {
                        st.placed.push(who);
                        moved.set(true);
                    }
                    Ok(())
                })?,
            )?;
            set(
                "remove",
                s.create_function(|_, who: String| {
                    let st = &mut sts.borrow_mut()[cur.get()];
                    let n = st.placed.len();
                    st.placed.retain(|p| *p != who);
                    moved.set(moved.get() || st.placed.len() != n);
                    Ok(())
                })?,
            )?;
            set(
                "pay",
                s.create_function(|_, n: u32| {
                    world.borrow_mut().credits += n;
                    Ok(())
                })?,
            )?;
            set(
                "charge",
                s.create_function(|_, n: u32| {
                    let mut w = world.borrow_mut();
                    let ok = w.credits >= n;
                    if ok {
                        w.credits -= n;
                    }
                    Ok(ok)
                })?,
            )?;
            set("give_cargo", s.create_function(|_, (g, n): (String, u32)| Ok(world.borrow_mut().take_cargo(good(&g)?, n)))?)?;
            set(
                "take_cargo",
                s.create_function(|_, (g, n): (String, u32)| {
                    let g = good(&g)?;
                    let mut w = world.borrow_mut();
                    let ok = w.cargo[g] >= n;
                    if ok {
                        w.cargo[g] -= n;
                    }
                    Ok(ok)
                })?,
            )?;
            set(
                "give_item",
                s.create_function(|_, item: String| {
                    world.borrow_mut().items.push(item);
                    Ok(())
                })?,
            )?;
            set(
                "take_item",
                s.create_function(|_, item: String| {
                    let mut w = world.borrow_mut();
                    let i = w.items.iter().position(|x| *x == item);
                    Ok(i.map(|i| w.items.remove(i)).is_some())
                })?,
            )?;
            set(
                "shift",
                s.create_function(|_, (f, n): (String, i32)| {
                    world.borrow_mut().standing.shift(faction(&f)?, n);
                    Ok(())
                })?,
            )?;
            set("spawn", s.create_function(|_, spec: Table| Ok(world.borrow_mut().spawn_tagged(&tagged(&spec)?)))?)?;
            set(
                "finish_",
                s.create_function(|_, text: String| {
                    end(Status::Done(text.clone()), text);
                    Ok(())
                })?,
            )?;
            set(
                "fail_",
                s.create_function(|_, text: String| {
                    end(Status::Failed(text.clone()), text);
                    Ok(())
                })?,
            )?;
            let random = s.create_function(|_, (a, b): (Option<i64>, Option<i64>)| {
                let mut w = world.borrow_mut();
                let (lo, hi) = match (a, b) {
                    (None, _) => return Ok(Value::Number(w.rng.range(0.0, 1.0) as f64)),
                    (Some(m), None) => (1, m),
                    (Some(m), Some(n)) => (m, n),
                };
                let span = hi as i128 - lo as i128 + 1;
                if span < 1 || span > u32::MAX as i128 {
                    return Err(mlua::Error::runtime("random: the interval is empty or too wide"));
                }
                Ok(Value::Integer((lo as i128 + w.rng.below(span as u32) as i128) as i64))
            })?;
            set("random", random)?;

            for (i, h) in handles.iter_mut().enumerate() {
                let Some(h) = h else { continue };
                cur.set(i);
                run(lua, h, i, &sts, budget);
            }
            Ok(())
        });
        if let Err(e) = r {
            log::error!("quests: {e}");
        }
        *states = sts.into_inner();
        *talk = queue.into_inner();
        *people_changed |= moved.get();
        heard.get()
    }
}

/// Runs quest `i` until it waits, finishes or breaks, through as many
/// stages as it moves on to.
fn run(lua: &Lua, h: &mut Handles, i: usize, sts: &RefCell<Vec<State>>, budget: &Rc<Cell<i64>>) {
    let broke = |e: String| {
        let mut sts = sts.borrow_mut();
        log::error!("quest {} broke in stage {}: {e}", sts[i].id, sts[i].stage);
        sts[i].status = Status::Broken(e);
    };
    for _ in 0..MAX_STAGES {
        if sts.borrow()[i].status != Status::Running {
            h.thread = None;
            return;
        }
        let th = match &h.thread {
            Some(t) => t.clone(),
            None => {
                let stage = sts.borrow()[i].stage.clone();
                let Ok(Value::Function(f)) = h.stages.get::<Value>(stage.as_str()) else {
                    return broke(format!("no stage called {stage}"));
                };
                let th = match lua.create_thread(f) {
                    Ok(t) => t,
                    Err(e) => return broke(e.to_string()),
                };
                let b = budget.clone();
                if let Err(e) = th.set_hook(HookTriggers::new().every_nth_instruction(HOOK_EVERY), move |_, _| spend(&b)) {
                    return broke(e.to_string());
                }
                h.thread = Some(th.clone());
                th
            }
        };
        budget.set(BUDGET);
        let r = th.resume::<Value>(());
        if sts.borrow()[i].status != Status::Running {
            h.thread = None;
            return;
        }
        match (r, th.status()) {
            (Ok(_), ThreadStatus::Resumable) => return,
            (Ok(Value::String(next)), ThreadStatus::Finished) => {
                let next = next.to_string_lossy();
                log::info!("quest {}: stage {next}", sts.borrow()[i].id);
                sts.borrow_mut()[i].stage = next;
                h.thread = None;
            }
            (Ok(Value::Nil), ThreadStatus::Finished) => {
                let mut s = sts.borrow_mut();
                log::info!("quest {}: ended", s[i].id);
                s[i].status = Status::Done(String::new());
                s[i].objective = None;
                h.thread = None;
                return;
            }
            (Ok(v), ThreadStatus::Finished) => {
                return broke(format!("a stage returned a {}, not the name of the next stage", v.type_name()));
            }
            (Err(e), _) => return broke(tidy(&e)),
            (Ok(_), st) => return broke(format!("the stage's coroutine is {st:?}")),
        }
    }
    broke(format!("passed through {MAX_STAGES} stages without waiting"))
}

/// An error as a quest's writer wants it, on one line: where, and what,
/// without mlua's preamble, the traceback or the folders.
fn tidy(e: &mlua::Error) -> String {
    // "/a/b/c.lua:3: ..." to "c.lua:3: ..."
    let short = |s: &str| -> String {
        match s.find(".lua:") {
            Some(i) => {
                let start = s[..i].rfind(['/', '\\', ' ', '[', '@']).map_or(0, |j| j + 1);
                s[start..].to_string()
            }
            None => s.to_string(),
        }
    };
    match e {
        mlua::Error::CallbackError { traceback, cause } => {
            // the first line of the script in the traceback is where it called
            let at = traceback.lines().map(str::trim).find(|l| l.contains(".lua:")).and_then(|l| l.split(": in ").next()).map(short);
            match at {
                Some(at) => format!("{at}: {}", tidy(cause)),
                None => tidy(cause),
            }
        }
        mlua::Error::RuntimeError(s) => short(s.lines().next().unwrap_or_default()),
        mlua::Error::SyntaxError { message, .. } => short(message.lines().next().unwrap_or_default()),
        mlua::Error::MemoryError(_) => "out of memory".into(),
        other => short(other.to_string().lines().next().unwrap_or_default()),
    }
}

fn spend(b: &Cell<i64>) -> mlua::Result<VmState> {
    let left = b.get() - 1;
    b.set(left);
    if left < 0 {
        return Err(mlua::Error::runtime("ran too long without waiting (a loop with no wait in it?)"));
    }
    Ok(VmState::Continue)
}

/// A Lua state with only the safe libraries, and the table every quest's
/// globals fall back on.
fn sandbox() -> mlua::Result<(Lua, Table, Table)> {
    let lua = Lua::new_with(StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::MATH, LuaOptions::default())?;
    lua.set_memory_limit(MEMORY)?;
    let g = lua.globals();
    // the base library's file readers, and anything that loads bytecode
    for k in ["dofile", "loadfile", "require", "collectgarbage"] {
        g.set(k, Value::Nil)?;
    }
    g.get::<Table>("string")?.set("dump", Value::Nil)?;
    g.get::<Table>("math")?.set("randomseed", Value::Nil)?;
    // the string library behind ("x"):upper() can't be reached and changed
    lua.load("getmetatable('').__metatable = false").set_mode(ChunkMode::Text).exec()?;
    // no coroutines of a quest's own: they would run outside the time cap
    lua.load("coroutine = { yield = coroutine.yield, isyieldable = coroutine.isyieldable }").set_mode(ChunkMode::Text).exec()?;
    // no finalizers: they run outside the time cap, as any quest
    lua.load(
        "local set, rawget, type, error = setmetatable, rawget, type, error
         setmetatable = function(t, mt)
           if type(mt) == 'table' and rawget(mt, '__gc') ~= nil then error('__gc is not allowed in quests', 2) end
           return set(t, mt)
         end",
    )
    .set_mode(ChunkMode::Text)
    .exec()?;
    let api = lua.create_table()?;
    for k in [
        "assert",
        "error",
        "ipairs",
        "next",
        "pairs",
        "pcall",
        "select",
        "tonumber",
        "tostring",
        "type",
        "xpcall",
        "rawequal",
        "rawget",
        "rawlen",
        "rawset",
        "setmetatable",
        "getmetatable",
        "coroutine",
        "table",
        "string",
        "math",
        "_VERSION",
    ] {
        api.set(k, g.get::<Value>(k)?)?;
    }
    // text only, and inside the sandbox unless given an environment
    let load = g.get::<Function>("load")?;
    let safe_api = api.clone();
    let safe = lua.create_function(move |lua, (chunk, name, _mode, env): (Value, Option<String>, Value, Option<Table>)| {
        let env = match env {
            Some(e) => e,
            None => {
                // globals of its own over the shared ones, like a quest's
                let e = lua.create_table()?;
                let meta = lua.create_table()?;
                meta.set("__index", safe_api.clone())?;
                meta.set("__metatable", false)?;
                e.set_metatable(Some(meta))?;
                e
            }
        };
        load.call::<Variadic<Value>>((chunk, name, "t", env))
    })?;
    api.set("load", safe.clone())?;
    g.set("load", safe)?;
    let print = lua.create_function(|_, args: Variadic<Value>| {
        let parts: Vec<String> = args.iter().map(|v| v.to_string().unwrap_or_else(|_| v.type_name().to_string())).collect();
        log::info!("quest: {}", parts.join("\t"));
        Ok(())
    })?;
    api.set("print", print.clone())?;
    api.set("log", print)?;
    // lasting wrappers over each tick's functions
    let calls = lua.create_table()?;
    let wrap: Function = lua
        .load("local calls, names = ...; for _, n in ipairs(names) do _ENV[n] = function(...) return calls[n](...) end end; math.random = random")
        .set_name("@wrappers")
        .set_mode(ChunkMode::Text)
        .set_environment(api.clone())
        .into_function()?;
    wrap.call::<()>((calls.clone(), CALLS.to_vec()))?;
    lua.load(PRELUDE).set_name("@prelude").set_mode(ChunkMode::Text).set_environment(api.clone()).exec()?;
    Ok((lua, api, calls))
}

fn built_in() -> Vec<Result<Def, (String, String)>> {
    BUILT_IN.iter().map(|(id, t, l)| Def::parse(id, t, l, &format!("data/quests/{id}")).map_err(|e| (id.to_string(), e))).collect()
}

/// The player's quests: each `<id>.toml` with a `<id>.lua` beside it.
fn read_dir(dir: &Path) -> Vec<Result<Def, (String, String)>> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let lua_path = p.with_extension("lua");
            let origin = p.with_extension("").display().to_string();
            let t = std::fs::read_to_string(&p).map_err(|e| (id.clone(), format!("{}: {e}", p.display())))?;
            let l = std::fs::read_to_string(&lua_path).map_err(|e| (id.clone(), format!("{}: {e}", lua_path.display())))?;
            log::info!("quest {id} from {}", p.display());
            Def::parse(&id, &t, &l, &origin).map_err(|e| (id, e))
        })
        .collect()
}

/// A faction by a short name (coop, crown, traders, inner, pirates) or its
/// full one.
pub fn faction_named(s: &str) -> Option<Faction> {
    let s = s.trim().to_lowercase();
    Some(match s.as_str() {
        "coop" | "cooperative" => Faction::Coop,
        "crown" => Faction::Crown,
        "traders" => Faction::Traders,
        "inner" | "customs" => Faction::Inner,
        "pirates" | "jumpers" => Faction::Pirates,
        _ => {
            return faction::ALL.into_iter().find(|f| f.name().to_lowercase() == s);
        }
    })
}

/// A room by name: Some(None) is the corridor (an empty name).
fn place_named(s: &str) -> Option<Option<Place>> {
    Some(Some(match s.trim().to_lowercase().as_str() {
        "" | "corridor" => return Some(None),
        "bar" => Place::Bar,
        "exchange" => Place::Exchange,
        "boards" | "office" => Place::Office,
        "yard" | "outfitter" | "shipyard" => Place::Yard,
        "hall" => Place::Hall,
        "hangar" => Place::Hangar,
        _ => return None,
    }))
}

fn faction(s: &str) -> mlua::Result<Faction> {
    faction_named(s).ok_or_else(|| mlua::Error::runtime(format!("no faction called {s}")))
}

/// A good by its name, any case, or by a word of it that only one good has
/// ("ice", "medicine").
pub fn good_named(s: &str) -> Option<usize> {
    let s = s.trim().to_lowercase();
    if let Some(i) = econ::GOODS.iter().position(|g| g.name.to_lowercase() == s) {
        return Some(i);
    }
    let hits: Vec<usize> = (0..econ::N).filter(|&i| econ::GOODS[i].name.to_lowercase().split([' ', '-']).any(|w| w == s)).collect();
    (hits.len() == 1).then(|| hits[0])
}

fn good(s: &str) -> mlua::Result<usize> {
    good_named(s).ok_or_else(|| mlua::Error::runtime(format!("no good called {s}")))
}

fn fold(s: &str) -> String {
    s.to_lowercase().replace('ä', "a").replace('ö', "o")
}

/// `spawn{ ... }`'s table, read.
fn tagged(t: &Table) -> mlua::Result<Tagged> {
    let tag: String = t.get::<Option<String>>("tag")?.ok_or_else(|| mlua::Error::runtime("spawn needs a tag"))?;
    let class_name = t.get::<Option<String>>("class")?.unwrap_or_else(|| "Kärppä".into());
    let class = ship::ALL_CLASSES.iter().copied().find(|c| fold(c.name) == fold(&class_name)).ok_or_else(|| mlua::Error::runtime(format!("no hull called {class_name}")))?;
    let side = match t.get::<Option<String>>("side")?.as_deref().unwrap_or("pirate") {
        "pirate" | "pirates" => Side::Pirate,
        "patrol" => Side::Patrol,
        "trader" => Side::Trader,
        other => {
            return Err(mlua::Error::runtime(format!("no side called {other} (pirate, patrol or trader)")));
        }
    };
    let faction = match t.get::<Option<String>>("faction")? {
        Some(f) => faction(&f)?,
        None => Faction::Coop,
    };
    Ok(Tagged {
        tag,
        class,
        side,
        faction,
        count: t.get::<Option<u32>>("count")?.unwrap_or(1),
        callsign: t.get("callsign")?,
        item: t.get("item")?,
        wreck: t.get::<Option<bool>>("wreck")?.unwrap_or(false),
        skill: t.get::<Option<f32>>("skill")?.unwrap_or(0.5).clamp(0.0, 1.0),
        distance: t.get::<Option<f32>>("distance")?.unwrap_or(3500.0).clamp(500.0, 8000.0),
    })
}

/// A quest's `vars`, for the save: numbers, strings, booleans, and tables
/// of them (lists, or maps with string keys). Anything else is dropped.
fn to_json(v: &Value, depth: u32) -> serde_json::Value {
    use serde_json::Value as J;
    if depth > 16 {
        return J::Null;
    }
    match v {
        Value::Boolean(b) => J::Bool(*b),
        Value::Integer(n) => J::from(*n),
        Value::Number(n) => serde_json::Number::from_f64(*n).map_or(J::Null, J::Number),
        Value::String(s) => J::String(s.to_string_lossy()),
        Value::Table(t) => {
            let n = t.raw_len();
            let count = t.pairs::<Value, Value>().count();
            if n > 0 && n == count {
                J::Array((1..=n).map(|i| to_json(&t.raw_get::<Value>(i).unwrap_or(Value::Nil), depth + 1)).collect())
            } else {
                let mut m = serde_json::Map::new();
                for (k, v) in t.pairs::<Value, Value>().flatten() {
                    if let Value::String(k) = k {
                        m.insert(k.to_string_lossy(), to_json(&v, depth + 1));
                    }
                }
                J::Object(m)
            }
        }
        _ => J::Null,
    }
}

fn from_json(lua: &Lua, v: &serde_json::Value, depth: u32) -> mlua::Result<Value> {
    use serde_json::Value as J;
    if depth > 16 {
        return Ok(Value::Nil);
    }
    Ok(match v {
        J::Null => Value::Nil,
        J::Bool(b) => Value::Boolean(*b),
        J::Number(n) => n.as_i64().map_or(Value::Number(n.as_f64().unwrap_or(0.0)), Value::Integer),
        J::String(s) => Value::String(lua.create_string(s)?),
        J::Array(a) => {
            let t = lua.create_table()?;
            for (i, x) in a.iter().enumerate() {
                t.raw_set(i + 1, from_json(lua, x, depth + 1)?)?;
            }
            Value::Table(t)
        }
        J::Object(m) => {
            let t = lua.create_table()?;
            for (k, x) in m {
                t.raw_set(k.as_str(), from_json(lua, x, depth + 1)?)?;
            }
            Value::Table(t)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(toml_text: &str, script: &str) -> Quests {
        Quests::new(vec![Def::parse("t", toml_text, script, "test").map_err(|e| ("t".to_string(), e))])
    }

    fn status(q: &Quests) -> Status {
        q.states[0].status.clone()
    }

    const PERSON: &str = r#"
title = "A test"
[people.ilta]
name = "Ilta Rask"
glyph = "i"
base = "Routa Station"
room = "bar"
idle = "Nothing yet."
[text]
hello = "You came."
"#;

    #[test]
    fn the_sandbox_holds() {
        let q = one("title = \"x\"", "return { start = function() wait(function() return false end) end }");
        let probe = |src: &str| -> String {
            let env = q.env().unwrap();
            let v: Value = q.lua.load(src).set_environment(env).eval().unwrap();
            if v.is_nil() { "nil".into() } else { v.type_name().into() }
        };
        for name in ["io", "os", "package", "debug", "dofile", "loadfile", "require", "collectgarbage", "string.dump", "math.randomseed"] {
            assert_eq!(probe(&format!("return {name}")), "nil", "{name} must not be reachable");
        }
        assert_eq!(probe("return string.rep"), "function", "the safe libraries are there");
        // load: text only, and in the sandbox
        assert_eq!(probe("return load('return 1')()"), "integer");
        assert_eq!(probe("return load('return io')()"), "nil", "loaded text sees no more than the quest");
        assert_eq!(probe("return (load('\\27Lua\\84\\0'))"), "nil", "bytecode is refused");
        assert_eq!(probe("return getmetatable('')"), "boolean", "the string library can't be reached through a string");
        assert_eq!(probe("return getmetatable(_ENV)"), "boolean", "nor the shared globals");
        assert_eq!(probe("return coroutine.wrap"), "nil", "no coroutines of a quest's own");
        assert_eq!(probe("return coroutine.create"), "nil");
        assert_eq!(probe("return pcall(setmetatable, {}, { __gc = function() end })"), "boolean");
        assert_eq!(probe("local ok = pcall(setmetatable, {}, { __gc = function() end }); return ok or nil"), "nil", "no finalizers");
    }

    #[test]
    fn a_runaway_or_greedy_script_breaks_only_itself() {
        let mut w = World::new(1);
        let mut q = one("title = \"loop\"", "return { start = function() while true do end end }");
        q.tick(&mut w, &Tick::default());
        assert!(matches!(status(&q), Status::Broken(ref e) if e.contains("too long")), "{:?}", status(&q));
        let mut q = one("title = \"greedy\"", "return { start = function() local t = {} for i = 1, 1e9 do t[i] = string.rep('x', 1000) .. i if i % 1000 == 0 then pause() end end end }");
        for _ in 0..200 {
            q.tick(&mut w, &Tick::default());
        }
        assert!(matches!(status(&q), Status::Broken(ref e) if e.contains("memory")), "{:?}", status(&q));
        let q = one("title = \"bad\"", "return 7");
        assert!(matches!(status(&q), Status::Broken(_)));
        let q = one("title = \"top\"", "while true do end");
        assert!(matches!(status(&q), Status::Broken(ref e) if e.contains("too long")), "a loop at the top of the file is stopped too");
    }

    #[test]
    fn one_quest_cannot_change_another() {
        let meddler = r#"return { start = function()
            wait = function() end
            string.rep = nil
            pcall(function() load('pay = nil; wait = nil')() end)
            pcall(function() getmetatable(_ENV).__index.wait = function() end end)
            pcall(function() getmetatable('').__index.rep = nil end)
            pause()
          end }"#;
        let honest = r#"return { start = function()
            wait(function() return vars.go end)
            pay(1)
            finish(string.rep("x", 2))
          end }"#;
        let defs = vec![
            Def::parse("m", "title = \"m\"", meddler, "m").map_err(|e| ("m".to_string(), e)),
            Def::parse("h", "title = \"h\"", honest, "h").map_err(|e| ("h".to_string(), e)),
        ];
        let mut q = Quests::new(defs);
        let mut w = World::new(1);
        for _ in 0..3 {
            q.tick(&mut w, &Tick::default());
        }
        assert_eq!(q.states[1].status, Status::Running, "still waiting, untouched");
        q.handles[1].as_ref().unwrap().env.get::<Table>("vars").unwrap().set("go", true).unwrap();
        q.tick(&mut w, &Tick::default());
        assert_eq!(q.states[1].status, Status::Done("xx".into()));
    }

    #[test]
    fn random_refuses_what_it_cannot_do() {
        let mut w = World::new(1);
        let mut q = one("title = \"r\"", "return { start = function() local a = random(1, 6); assert(a >= 1 and a <= 6); assert(math.random(3) <= 3); random(math.mininteger, math.maxinteger) end }");
        q.tick(&mut w, &Tick::default());
        assert!(matches!(status(&q), Status::Broken(ref e) if e.contains("too wide")), "{:?}", status(&q));
    }

    #[test]
    fn a_broken_quest_keeps_its_progress() {
        let mut w = World::new(1);
        // broke as it ran: saved where it was
        let mut q = one("title = \"b\"", "return { start = function() vars.k = 1; return 'two' end, two = function() error('oops') end }");
        q.tick(&mut w, &Tick::default());
        assert!(matches!(status(&q), Status::Broken(_)));
        let saved = q.save();
        assert_eq!((saved[0].stage.as_str(), &saved[0].status), ("two", &Status::Running));
        // didn't load at all: its saved entry is kept as it was
        let mut q2 = one("title = \"b\"", "return {");
        q2.restore(&saved);
        assert_eq!(q2.save(), saved);
    }

    #[test]
    fn a_quest_reads_in_order_and_waits_for_the_world() {
        let mut w = World::new(1);
        let mut q = one(
            "title = \"Run\"",
            r#"return {
              start = function()
                objective("Fly to Kide Mine")
                vars.asked = (vars.asked or 0) + 1
                wait_flying()
                return "fly"
              end,
              fly = function()
                wait_docked("Kide Mine")
                pay(500)
                finish("Delivered.")
              end,
            }"#,
        );
        q.tick(&mut w, &Tick::default());
        assert_eq!(q.states[0].objective.as_deref(), Some("Fly to Kide Mine"));
        assert_eq!(q.states[0].stage, "start");
        w.launch();
        q.tick(&mut w, &Tick::default());
        assert_eq!(q.states[0].stage, "fly");
        let credits = w.credits;
        q.tick(&mut w, &Tick::default());
        assert_eq!(w.credits, credits, "not there yet");
        // there: in Kide Drift, docked
        w.region = 1;
        w.enter_region_for_test(1);
        w.phase = Phase::Docked(0);
        q.tick(&mut w, &Tick::default());
        assert_eq!(w.credits, credits + 500);
        assert_eq!(status(&q), Status::Done("Delivered.".into()));
        assert!(q.states[0].objective.is_none());
        assert!(w.comms.iter().any(|c| c.text == "Delivered."), "said in flight on the comms");
    }

    #[test]
    fn a_saved_quest_starts_its_stage_again_with_its_vars() {
        let script = r#"return {
              start = function() vars.n = 3; vars.list = {"a", "b"}; vars.map = {k = true}; return "two" end,
              two = function() vars.n = vars.n + 1; wait(function() return false end) end,
            }"#;
        let mut w = World::new(1);
        let mut q = one("title = \"S\"", script);
        q.tick(&mut w, &Tick::default());
        let saved = q.save();
        assert_eq!(saved[0].stage, "two");
        assert_eq!(saved[0].vars, serde_json::json!({"n": 4, "list": ["a", "b"], "map": {"k": true}}));
        // through JSON and back, as the save file does
        let saved: Vec<Saved> = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        let mut q2 = one("title = \"S\"", script);
        q2.restore(&saved);
        q2.tick(&mut w, &Tick::default());
        assert_eq!(q2.save()[0].vars["n"], 5, "the stage ran again from its top on the saved vars");
        // a quest whose files are gone keeps its progress in the save
        let mut q3 = one("title = \"other\"", "return { start = function() pause() end }");
        let mut gone = saved.clone();
        gone[0].id = "gone".into();
        q3.restore(&gone);
        assert!(q3.save().iter().any(|s| s.id == "gone"));
    }

    #[test]
    fn people_talk_and_answer_on_foot() {
        let mut w = World::new(1);
        let mut q = one(
            PERSON,
            r#"return {
              start = function()
                place("ilta")
                objective("Find Ilta in Routa's bar")
                wait_talk("ilta")
                say("ilta", text.hello)
                local a = choose("ilta", "Will you do it?", {"Yes", "No"})
                if a == 1 then give_item("beacon"); finish("She thanks you.") else fail("She walks off.") end
              end,
            }"#,
        );
        let foot = Tick { on_foot: true, talked: None };
        q.tick(&mut w, &foot);
        assert!(q.people_changed);
        let people = q.people_at("Routa Station");
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].0.name, "Ilta Rask");
        assert_eq!(people[0].2, Some(Place::Bar));
        assert!(q.people_at("Kide Mine").is_empty());
        // someone else's name is not heard
        assert!(!q.tick(&mut w, &Tick { on_foot: true, talked: Some("someone".into()) }));
        assert!(q.tick(&mut w, &Tick { on_foot: true, talked: Some("ilta".into()) }), "the quest was listening");
        assert_eq!(q.talk.len(), 2);
        assert_eq!(q.talk[0].who, "Ilta Rask");
        assert_eq!(q.talk[0].text, "You came.");
        assert!(!q.answer(1), "a line is read, not answered");
        q.dismiss();
        assert_eq!(q.talk[0].choices, vec!["Yes", "No"]);
        q.dismiss();
        assert_eq!(q.talk.len(), 1, "a question waits for its answer");
        assert!(!q.answer(3));
        assert!(q.answer(1));
        q.tick(&mut w, &foot);
        assert_eq!(status(&q), Status::Done("She thanks you.".into()));
        assert_eq!(w.items, vec!["beacon".to_string()]);
        assert!(q.people_at("Routa Station").is_empty(), "a finished quest's people leave");
        assert_eq!(q.talk.back().map(|t| t.text.as_str()), Some("She thanks you."));
    }

    #[test]
    fn quests_spawn_ships_and_see_them_go() {
        let mut w = World::new(1);
        let mut q = one(
            "title = \"Hunt\"",
            r#"return {
              start = function()
                wait_flying()
                assert(spawn{ tag = "jumpers", class = "Karppa", count = 2, item = "claim beacon" } == 2)
                wait(function() return alive("jumpers") == 0 end)
                finish("")
              end,
            }"#,
        );
        q.tick(&mut w, &Tick::default());
        assert_eq!(w.tagged("jumpers"), (0, 0), "not while docked");
        w.launch();
        q.tick(&mut w, &Tick::default());
        assert_eq!(w.tagged("jumpers"), (2, 0));
        assert_eq!(w.ships.iter().filter(|s| s.item.is_some()).count(), 1);
        // fly far off: the others leave the region, the one with the item stays
        w.player_mut_for_test().pos = crate::sim::v2::v2(60000.0, 60000.0);
        w.update(&crate::sim::ship::Intent::default(), 1.0 / 60.0);
        assert_eq!(w.tagged("jumpers").0, 1);
        assert!(w.ships.iter().any(|s| s.item.is_some() && s.alive));
        for s in w.ships.iter_mut().filter(|s| s.tag.is_some()) {
            s.alive = false;
        }
        q.tick(&mut w, &Tick::default());
        assert_eq!(status(&q), Status::Done(String::new()));
    }

    #[test]
    fn mistakes_in_a_quest_say_where() {
        let mut w = World::new(1);
        let mut q = one("title = \"oops\"", "return {\n start = function()\n  cargo('unobtainium')\n end }");
        q.tick(&mut w, &Tick::default());
        let Status::Broken(e) = status(&q) else { panic!() };
        assert!(e.contains("test.lua:3") && e.contains("no good called unobtainium"), "{e}");
        assert!(Def::parse("t", "title = 1", "", "x").is_err());
        assert!(Def::parse("t", "title = \"x\"\n[people.a]\nname = \"A\"\nglyph = \"a\"\nbase = \"B\"\nroom = \"spa\"", "", "x").is_err());
        assert_eq!(good_named("ice"), Some(0));
        assert_eq!(good_named("AI cores"), Some(11));
        assert_eq!(good_named("parts"), Some(7));
    }

    #[test]
    fn the_built_in_quests_load() {
        let q = Quests::built_in();
        for s in &q.states {
            assert_eq!(s.status, Status::Running, "{}", s.id);
        }
    }
}
