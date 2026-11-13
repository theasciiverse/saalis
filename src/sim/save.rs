//! Saved games. As in Ascii Sector, you save while docked; a save puts you
//! back at that base. One slot, written on every landing and from the dock's
//! menu, as versioned JSON (Armoured Commander II's pattern) in the
//! platform's data folder (Glider's `save::dir`). A save of another version
//! is not offered.

use super::econ::{self, Market};
use super::ship::{self, Ship, Side};
use super::{Phase, World, PLAYER};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const FORMAT: u32 = 6;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Save {
    pub format: u32,
    pub seed: u64,
    pub region: usize,
    pub base: usize,
    pub clock: f64,
    pub credits: u32,
    pub cargo: [u32; econ::N],
    pub paid: [u32; econ::N],
    pub hull_class: String,
    pub kit: super::ship::Kit,
    pub armour: [f32; 4],
    pub hull: f32,
    pub kills: u32,
    pub encounters: u32,
    pub markets: Vec<Market>,
    pub standing: super::faction::Standing,
    pub missions: Vec<super::mission::Mission>,
    pub mission_cargo: u32,
    pub members: [bool; 2],
    pub rating: [u32; 2],
    pub next_mission: u32,
    pub war: super::war::War,
    /// you on foot (0.7; older saves get the starting kit)
    #[serde(default)]
    pub body: super::body::Body,
    /// what quests gave you (0.8)
    #[serde(default)]
    pub items: Vec<String>,
    /// the quests' progress (0.8), filled in by the app
    #[serde(default)]
    pub quests: Vec<super::quest::Saved>,
    /// the quests' shared flags (0.9)
    #[serde(default)]
    pub flags: std::collections::BTreeMap<String, String>,
    /// lanes the story opened (0.9)
    #[serde(default)]
    pub opened: Vec<(usize, usize)>,
}

/// Where saves live: `SAALIS_DATA_DIR`, or the state folder on Linux
/// (`$XDG_STATE_HOME/saalis`, the Asciiverse convention, beside the session
/// log), Application Support on macOS, `%APPDATA%` on Windows.
pub fn dir() -> Option<PathBuf> {
    // tests never touch a player's saves (a test once overwrote one)
    if cfg!(test) {
        return None;
    }
    if let Some(d) = std::env::var_os("SAALIS_DATA_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("saalis"));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/saalis"));
    }
    std::env::var_os("XDG_STATE_HOME").filter(|d| !d.is_empty()).map(PathBuf::from).or_else(|| home.map(|h| h.join(".local/state"))).map(|d| d.join("saalis"))
}

/// Where Linux saves lived up to 0.8 (`$XDG_DATA_HOME/saalis`).
fn old_dir() -> Option<PathBuf> {
    if cfg!(test) || cfg!(windows) || cfg!(target_os = "macos") || std::env::var_os("SAALIS_DATA_DIR").is_some_and(|d| !d.is_empty()) {
        return None;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()).map(PathBuf::from).or_else(|| home.map(|h| h.join(".local/share"))).map(|d| d.join("saalis"))
}

/// Copies a save and quests from the old Linux folder into the state
/// folder, once, when the state folder has no save yet. The old folder is
/// left as it was.
pub fn migrate() {
    let (Some(old), Some(new)) = (old_dir(), dir()) else { return };
    if old == new || new.join("save.json").exists() || !old.join("save.json").exists() {
        return;
    }
    if std::fs::create_dir_all(&new).is_err() {
        return;
    }
    let ok = std::fs::copy(old.join("save.json"), new.join("save.json")).is_ok();
    if let Ok(quests) = std::fs::read_dir(old.join("quests")) {
        let _ = std::fs::create_dir_all(new.join("quests"));
        for f in quests.flatten() {
            if f.path().is_file() && !new.join("quests").join(f.file_name()).exists() {
                let _ = std::fs::copy(f.path(), new.join("quests").join(f.file_name()));
            }
        }
    }
    log::info!("save copied from {} to {}: {ok}", old.display(), new.display());
}

fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("save.json"))
}

impl World {
    /// The game as a save, if docked.
    pub fn to_save(&self) -> Option<Save> {
        let Phase::Docked(base) = self.phase else { return None };
        let p = self.ships.iter().find(|s| s.id == PLAYER)?;
        Some(Save {
            format: FORMAT,
            seed: self.seed,
            region: self.region,
            base,
            clock: self.clock,
            credits: self.credits,
            cargo: self.cargo,
            paid: self.paid,
            hull_class: p.class.name.to_string(),
            kit: p.kit.clone(),
            armour: p.armour,
            hull: p.hull,
            kills: self.kills,
            encounters: self.encounters,
            markets: self.markets.clone(),
            standing: self.standing.clone(),
            missions: self.missions.clone(),
            mission_cargo: self.mission_cargo,
            members: self.members,
            rating: self.rating,
            next_mission: self.next_mission,
            war: self.war.clone(),
            body: self.body.clone(),
            items: self.items.clone(),
            quests: Vec::new(),
            flags: self.flags.clone(),
            opened: self.opened.clone(),
        })
    }

    /// A world from a save: docked where it was saved.
    pub fn from_save(s: Save) -> Option<World> {
        if s.format != FORMAT {
            return None;
        }
        let mut w = World::new(s.seed);
        // a save from before the Reach grew has fewer markets: the new
        // bases come last, and start fresh
        if s.region >= w.sector.regions.len() || s.base >= w.sector.regions[s.region].bases.len() || s.markets.len() > w.markets.len() {
            return None;
        }
        for &(a, b) in &s.opened {
            w.sector.open_lane(a, b);
        }
        w.opened = s.opened;
        w.flags = s.flags;
        w.enter_region(s.region);
        let laws: Vec<_> = w.sector.all_bases().iter().map(|&(r, b)| (w.sector.regions[r].bases[b].kind, w.sector.regions[r].owner, w.sector.regions[r].bases[b].black_market)).collect();
        let fresh = w.markets.split_off(s.markets.len());
        w.markets = s.markets.into_iter().zip(laws).map(|(m, (k, o, bm))| m.with_kind(k, o, bm)).chain(fresh).collect();
        w.clock = s.clock;
        w.credits = s.credits;
        w.cargo = s.cargo;
        w.paid = s.paid;
        w.kills = s.kills;
        w.encounters = s.encounters;
        w.standing = s.standing;
        w.missions = s.missions;
        w.mission_cargo = s.mission_cargo;
        w.members = s.members;
        w.rating = s.rating;
        w.next_mission = s.next_mission;
        w.body = s.body;
        w.items = s.items;
        if s.war.control.len() <= w.sector.regions.len() {
            w.war = s.war.grown(&w.sector);
        }
        let class = ship::hull_named(&s.hull_class)?;
        let mut p = Ship::new(PLAYER, class, Side::Player, "you", w.bases[s.base].pos, 0.0);
        p.refit(s.kit);
        p.armour = s.armour;
        p.hull = s.hull.max(1.0);
        p.vel = super::v2::V2::ZERO;
        p.set_speed = 0.0;
        w.ships = vec![p];
        w.phase = Phase::Docked(s.base);
        w.refresh_board();
        Some(w)
    }
}

/// Writes the save; returns whether it worked.
pub fn write(s: &Save) -> bool {
    let Some(p) = path() else { return false };
    let ok = p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok())
        && serde_json::to_string(s).ok().is_some_and(|j| {
            let tmp = p.with_extension("tmp");
            std::fs::write(&tmp, j).is_ok() && std::fs::rename(&tmp, &p).is_ok()
        });
    if ok {
        log::info!("saved to {}", p.display());
    } else {
        log::warn!("could not save to {}", p.display());
    }
    ok
}

/// The save, if there is a readable one of this format.
pub fn read() -> Option<Save> {
    let p = path()?;
    let text = std::fs::read_to_string(&p).ok()?;
    match serde_json::from_str::<Save>(&text) {
        Ok(s) if s.format == FORMAT => Some(s),
        Ok(s) => {
            log::warn!("save format {} is not {FORMAT}; ignored", s.format);
            None
        }
        Err(e) => {
            log::warn!("unreadable save {}: {e}", p.display());
            None
        }
    }
}

/// A short line describing the save, for the title.
pub fn describe(s: &Save) -> String {
    let sector = super::sector::Sector::reach();
    let base = sector.regions.get(s.region).and_then(|r| r.bases.get(s.base)).map_or("?", |b| b.name);
    let day = (s.clock / 24.0).floor() as u64 + 1;
    format!("{base}, day {day}, {} cr", s.credits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_round_trips() {
        let mut w = World::new(9);
        w.buy(3, 4);
        w.clock = 50.5;
        let s = w.to_save().expect("docked");
        let json = serde_json::to_string(&s).unwrap();
        let back: Save = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        let w2 = World::from_save(back).expect("loads");
        assert_eq!(w2.cargo, w.cargo);
        assert_eq!(w2.credits, w.credits);
        assert_eq!(w2.markets[0].buy_price(3), w.markets[0].buy_price(3), "the market remembers the sale");
        assert_eq!(w2.phase, Phase::Docked(0));
    }

    #[test]
    fn flying_is_not_a_save_point() {
        let mut w = World::new(9);
        w.launch();
        assert!(w.to_save().is_none());
    }

    #[test]
    fn another_format_is_refused() {
        let w = World::new(9);
        let mut s = w.to_save().unwrap();
        s.format = 999;
        assert!(World::from_save(s).is_none());
    }

    #[test]
    fn a_save_from_the_smaller_reach_loads() {
        // 0.8 had nine regions and seven bases
        let mut w = World::new(9);
        w.markets[1].stock[0] = 3.0;
        let mut s = w.to_save().unwrap();
        s.markets.truncate(7);
        s.war.control.truncate(9);
        s.war.garrison.truncate(9);
        let w2 = World::from_save(s).expect("it loads");
        assert_eq!(w2.markets.len(), w.markets.len());
        assert_eq!(w2.markets[1].stock[0], 3.0);
        assert_eq!(w2.war.control.len(), w2.sector.regions.len());
    }

    #[test]
    fn opened_lanes_and_flags_are_kept() {
        let mut w = World::new(9);
        let dawn = w.sector.region_named("Aamunkoi").unwrap();
        assert!(w.open_lane(1, dawn));
        w.flags.insert("ending".into(), "coop".into());
        let w2 = World::from_save(w.to_save().unwrap()).unwrap();
        assert!(w2.sector.shown(dawn) && !w2.sector.route(0, dawn).is_empty());
        assert_eq!(w2.flags.get("ending").map(String::as_str), Some("coop"));
    }
}
