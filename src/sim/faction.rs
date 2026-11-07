//! Who flies the Reach (David, 2026-11-07: the low-belt corporate crown,
//! the Reach's mining cooperatives, free traders, pirates and
//! claim-jumpers, and the inner planets as an opposing force). Names are
//! placeholders until the Reach's own are settled.
//!
//! Standing runs from -125 to 125, as Ascii Sector's attitude does: at -25
//! or below a faction's patrols shoot on sight, and at -40 its bases won't
//! let you land.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Faction {
    /// the Reach's mining cooperatives and their militia, the Reach Watch
    Coop,
    /// the low belt's corporate crown
    Crown,
    /// the free traders and their guild
    Traders,
    /// the inner planets' navy and customs
    Inner,
    /// claim-jumpers and pirates
    Pirates,
}

pub const ALL: [Faction; 5] = [Faction::Coop, Faction::Crown, Faction::Traders, Faction::Inner, Faction::Pirates];

pub const MIN: i32 = -125;
pub const MAX: i32 = 125;
/// At or below this, a faction's patrols attack you.
pub const HOSTILE: i32 = -25;
/// At or below this, its bases refuse you.
pub const BANNED: i32 = -40;

impl Faction {
    pub fn name(self) -> &'static str {
        match self {
            Faction::Coop => "Reach Cooperative",
            Faction::Crown => "Crown Combine",
            Faction::Traders => "Free Traders",
            Faction::Inner => "Inner Customs",
            Faction::Pirates => "claim-jumpers",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether two non-player factions fight on sight: pirates against
    /// everyone else.
    pub fn at_war(self, o: Faction) -> bool {
        self != o && (self == Faction::Pirates || o == Faction::Pirates)
    }
}

/// The player's standing with every faction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Standing(pub [i32; 5]);

impl Default for Standing {
    fn default() -> Standing {
        // a Reach-born pilot: liked at home, unknown below, hunted by pirates
        Standing([10, 0, 5, -5, -30])
    }
}

impl Standing {
    pub fn of(&self, f: Faction) -> i32 {
        self.0[f.index()]
    }

    pub fn shift(&mut self, f: Faction, by: i32) {
        let v = &mut self.0[f.index()];
        *v = (*v + by).clamp(MIN, MAX);
    }

    /// A word for a standing.
    pub fn word(v: i32) -> &'static str {
        match v {
            i32::MIN..=-60 => "hated",
            -59..=-25 => "hostile",
            -24..=-6 => "wary",
            -5..=5 => "neutral",
            6..=30 => "friendly",
            31..=70 => "trusted",
            _ => "honoured",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pirates_fight_everyone_else_does_not() {
        assert!(Faction::Pirates.at_war(Faction::Coop));
        assert!(Faction::Inner.at_war(Faction::Pirates));
        assert!(!Faction::Crown.at_war(Faction::Inner));
        assert!(!Faction::Pirates.at_war(Faction::Pirates));
    }

    #[test]
    fn standing_is_bounded() {
        let mut s = Standing::default();
        s.shift(Faction::Coop, 1000);
        assert_eq!(s.of(Faction::Coop), MAX);
        s.shift(Faction::Coop, -1000);
        assert_eq!(s.of(Faction::Coop), MIN);
    }
}
