//! You on foot: health, a sidearm, a vest and medkits. Sold at outfitters
//! (and medkits at clinics too). Health comes back in full at every
//! landing. Names and numbers are placeholders, to tune by play.

use serde::{Deserialize, Serialize};

pub const MAX_HP: i32 = 30;
pub const MEDKIT_HEAL: i32 = 15;
pub const MAX_MEDKITS: u8 = 5;
pub const MEDKIT_PRICE: u32 = 80;

/// A hand weapon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    pub name: &'static str,
    /// damage, lowest and highest
    pub dmg: (i32, i32),
    /// the chance to hit next to the target; it falls with distance
    pub aim: f32,
    /// in tiles
    pub range: i32,
    pub price: u32,
}

pub const ARMS: [Arm; 3] = [
    Arm { name: "pistol", dmg: (4, 8), aim: 0.8, range: 8, price: 0 },
    Arm { name: "carbine", dmg: (6, 11), aim: 0.85, range: 12, price: 900 },
    Arm { name: "scattergun", dmg: (9, 15), aim: 0.75, range: 5, price: 1200 },
];

/// A vest: its name, the damage it takes off every hit, its price.
pub const VESTS: [(&str, i32, u32); 3] = [("no vest", 0, 0), ("padded vest", 2, 400), ("plated vest", 4, 1400)];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub hp: i32,
    /// index into `ARMS`
    pub arm: u8,
    /// index into `VESTS`
    pub vest: u8,
    pub medkits: u8,
}

impl Default for Body {
    fn default() -> Body {
        Body { hp: MAX_HP, arm: 0, vest: 0, medkits: 1 }
    }
}

impl Body {
    pub fn arm(&self) -> &'static Arm {
        &ARMS[(self.arm as usize).min(ARMS.len() - 1)]
    }

    pub fn soak(&self) -> i32 {
        VESTS[(self.vest as usize).min(VESTS.len() - 1)].1
    }

    pub fn vest_name(&self) -> &'static str {
        VESTS[(self.vest as usize).min(VESTS.len() - 1)].0
    }

    /// Uses a medkit if there is one and it would help.
    pub fn heal(&mut self) -> bool {
        if self.medkits == 0 || self.hp >= MAX_HP {
            return false;
        }
        self.medkits -= 1;
        self.hp = (self.hp + MEDKIT_HEAL).min(MAX_HP);
        true
    }
}
