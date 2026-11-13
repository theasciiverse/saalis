//! Hull classes, ships and the flight model.
//!
//! Flight is Privateer-style (David, 2026-11-07): the ship turns at its turn
//! rate, and its velocity moves toward heading x set speed at the ship's
//! acceleration. Turning hard therefore drifts for a moment before the new
//! heading takes. The afterburner raises the target speed and the
//! acceleration while its fuel lasts.
//!
//! Defence follows Ascii Sector's model: one energy pool feeds the guns and
//! recharges four shield facings; under each facing is armour that does not
//! recharge; under the armour, the hull.

use super::faction::Faction;
use super::v2::{turn_to, V2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Front,
    Rear,
    Left,
    Right,
}

impl Facing {
    pub const ALL: [Facing; 4] = [Facing::Front, Facing::Rear, Facing::Left, Facing::Right];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn letter(self) -> char {
        match self {
            Facing::Front => 'F',
            Facing::Rear => 'B',
            Facing::Left => 'L',
            Facing::Right => 'R',
        }
    }

    /// Which facing a blow lands on, for a ship with `heading` struck from
    /// the direction `from` (a heading, pointing from the ship to the source).
    pub fn struck(heading: f32, from: f32) -> Facing {
        let rel = turn_to(heading, from);
        let q = std::f32::consts::FRAC_PI_4;
        if rel.abs() <= q {
            Facing::Front
        } else if rel.abs() >= 3.0 * q {
            Facing::Rear
        } else if rel > 0.0 {
            Facing::Right
        } else {
            Facing::Left
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gun {
    pub name: &'static str,
    pub damage: f32,
    pub energy: f32,
    /// seconds between shots
    pub refire: f32,
    /// muzzle speed, m/s, added to the ship's own velocity
    pub speed: f32,
    pub range: f32,
}

/// Guns a shipyard sells, with their price per mount.
pub const PLASMA: Gun = Gun { name: "plasma gun", damage: 17.0, energy: 11.0, refire: 0.5, speed: 780.0, range: 1000.0 };
pub const SHOP_GUNS: [(Gun, u32); 3] = [(LASER, 600), (DRIVER, 1500), (PLASMA, 3800)];

pub const LASER: Gun = Gun { name: "laser", damage: 6.0, energy: 3.0, refire: 0.22, speed: 1100.0, range: 1300.0 };
pub const DRIVER: Gun = Gun { name: "mass driver", damage: 10.0, energy: 5.5, refire: 0.34, speed: 850.0, range: 1100.0 };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Light,
    Medium,
    Heavy,
}

/// A hull class. Names are placeholders until the Reach's own ship names are
/// settled.
#[derive(Debug, PartialEq)]
pub struct Class {
    pub name: &'static str,
    /// one line for the target box
    pub role: &'static str,
    pub size: Size,
    pub max_speed: f32,
    pub burner_speed: f32,
    /// m/s per second
    pub accel: f32,
    /// radians per second
    pub turn_rate: f32,
    pub energy: f32,
    /// energy per second
    pub recharge: f32,
    /// per facing
    pub shield: f32,
    /// shield points per second per facing (drawn from energy)
    pub shield_regen: f32,
    /// per facing: front, rear, left, right
    pub armour: [f32; 4],
    pub hull: f32,
    /// seconds of afterburner
    pub fuel: f32,
    pub guns: &'static [Gun],
    /// collision and hit radius, metres
    pub radius: f32,
    /// bounty, credits
    pub bounty: u32,
    /// cargo hold, units
    pub cargo: u32,
}

pub static TIIRA: Class = Class {
    name: "Tiira",
    role: "light courier-fighter",
    size: Size::Light,
    max_speed: 280.0,
    burner_speed: 560.0,
    accel: 230.0,
    turn_rate: 2.8,
    energy: 100.0,
    recharge: 20.0,
    shield: 22.0,
    shield_regen: 4.0,
    armour: [18.0, 14.0, 14.0, 14.0],
    hull: 40.0,
    fuel: 6.0,
    guns: &[LASER, LASER],
    radius: 30.0,
    bounty: 0,
    cargo: 30,
};

pub static KARPPA: Class = Class {
    name: "Kärppä",
    role: "claim-jumper raider",
    size: Size::Light,
    max_speed: 300.0,
    burner_speed: 520.0,
    accel: 260.0,
    turn_rate: 3.0,
    energy: 60.0,
    recharge: 14.0,
    shield: 10.0,
    shield_regen: 2.0,
    armour: [8.0, 6.0, 6.0, 6.0],
    hull: 20.0,
    fuel: 4.0,
    guns: &[LASER],
    radius: 28.0,
    bounty: 150,
    cargo: 0,
};

pub static AHMA: Class = Class {
    name: "Ahma",
    role: "pirate heavy fighter",
    size: Size::Medium,
    max_speed: 250.0,
    burner_speed: 470.0,
    accel: 190.0,
    turn_rate: 2.2,
    energy: 90.0,
    recharge: 18.0,
    shield: 16.0,
    shield_regen: 3.0,
    armour: [16.0, 12.0, 12.0, 12.0],
    hull: 35.0,
    fuel: 4.0,
    guns: &[DRIVER, DRIVER],
    radius: 34.0,
    bounty: 400,
    cargo: 0,
};

pub static KARHU: Class = Class {
    name: "Karhu",
    role: "pirate gunboat",
    size: Size::Heavy,
    max_speed: 170.0,
    burner_speed: 300.0,
    accel: 110.0,
    turn_rate: 1.3,
    energy: 160.0,
    recharge: 26.0,
    shield: 26.0,
    shield_regen: 4.0,
    armour: [30.0, 22.0, 26.0, 26.0],
    hull: 80.0,
    fuel: 3.0,
    guns: &[LASER, DRIVER, LASER],
    radius: 48.0,
    bounty: 1200,
    cargo: 0,
};

pub static KUIKKA: Class = Class {
    name: "Kuikka",
    role: "medium trader",
    size: Size::Medium,
    max_speed: 250.0,
    burner_speed: 470.0,
    accel: 190.0,
    turn_rate: 2.2,
    energy: 120.0,
    recharge: 22.0,
    shield: 26.0,
    shield_regen: 4.0,
    armour: [22.0, 18.0, 18.0, 18.0],
    hull: 60.0,
    fuel: 6.0,
    guns: &[LASER, LASER],
    radius: 36.0,
    bounty: 0,
    cargo: 70,
};

pub static LOKKI: Class = Class {
    name: "Lokki",
    role: "bulk hauler",
    size: Size::Heavy,
    max_speed: 200.0,
    burner_speed: 360.0,
    accel: 120.0,
    turn_rate: 1.5,
    energy: 140.0,
    recharge: 24.0,
    shield: 30.0,
    shield_regen: 4.0,
    armour: [28.0, 26.0, 26.0, 26.0],
    hull: 90.0,
    fuel: 5.0,
    guns: &[LASER, LASER],
    radius: 46.0,
    bounty: 0,
    cargo: 150,
};

pub static HAUKKA: Class = Class {
    name: "Haukka",
    role: "heavy fighter",
    size: Size::Medium,
    max_speed: 300.0,
    burner_speed: 600.0,
    accel: 260.0,
    turn_rate: 2.7,
    energy: 160.0,
    recharge: 32.0,
    shield: 30.0,
    shield_regen: 5.0,
    armour: [26.0, 20.0, 22.0, 22.0],
    hull: 60.0,
    fuel: 7.0,
    guns: &[DRIVER, LASER, LASER, DRIVER],
    radius: 34.0,
    bounty: 0,
    cargo: 25,
};

/// Hulls a shipyard sells the player, with prices; a trade-in fetches 60%.
pub static PLAYER_HULLS: [(&Class, u32); 4] = [(&TIIRA, 6000), (&KUIKKA, 14000), (&LOKKI, 22000), (&HAUKKA, 30000)];

/// Every hull, the player's and everyone else's.
pub static ALL_CLASSES: [&Class; 9] = [
    &TIIRA, &KARPPA, &AHMA, &KARHU, &KUIKKA, &LOKKI, &HAUKKA, &KOTKA, &HAAHKA,
];

/// A player hull by name, for loading a save.
pub fn hull_named(name: &str) -> Option<&'static Class> {
    PLAYER_HULLS.iter().map(|h| h.0).find(|c| c.name == name)
}

/// Which hulls carry a missile launcher and a rear turret mount.
pub fn mounts(c: &Class) -> (bool, bool) {
    match c.name {
        "Tiira" => (true, false),
        "Kuikka" => (true, true),
        "Lokki" => (false, true),
        "Haukka" => (true, true),
        _ => (false, false),
    }
}

pub static KOTKA: Class = Class {
    name: "Kotka",
    role: "patrol fighter",
    size: Size::Medium,
    max_speed: 290.0,
    burner_speed: 540.0,
    accel: 240.0,
    turn_rate: 2.6,
    energy: 110.0,
    recharge: 22.0,
    shield: 20.0,
    shield_regen: 4.0,
    armour: [18.0, 14.0, 14.0, 14.0],
    hull: 40.0,
    fuel: 5.0,
    guns: &[LASER, LASER],
    radius: 32.0,
    bounty: 0,
    cargo: 0,
};

pub static HAAHKA: Class = Class {
    name: "Haahka",
    role: "bulk freighter",
    size: Size::Heavy,
    max_speed: 160.0,
    burner_speed: 260.0,
    accel: 80.0,
    turn_rate: 0.9,
    energy: 120.0,
    recharge: 18.0,
    shield: 30.0,
    shield_regen: 3.0,
    armour: [30.0, 30.0, 30.0, 30.0],
    hull: 90.0,
    fuel: 3.0,
    guns: &[LASER],
    radius: 52.0,
    bounty: 0,
    cargo: 200,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Player,
    Pirate,
    /// a faction's militia, on patrol
    Patrol,
    /// a freighter going about its business
    Trader,
}

/// What a pilot (person, autopilot or AI) asks of the ship this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    /// -1 left .. 1 right, of the turn rate
    pub turn: f32,
    /// change of set speed, -1..1 of max speed per second-and-a-half
    pub throttle: f32,
    /// an absolute set speed instead, m/s (AI, Home, End, match speed)
    pub set_speed: Option<f32>,
    pub burner: bool,
    pub fire: bool,
    /// launch a missile (seeking if locked on and carried, else dumbfire)
    pub missile: bool,
}

/// What a hit did, for the sound and the comms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Struck {
    Shield,
    Armour,
    Hull,
    Destroyed,
}

/// What a ship carries beyond its bare hull: the shipyard's work. Grades
/// run 1 (as built) to 3.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kit {
    #[serde(with = "gun_names")]
    pub guns: Vec<Gun>,
    pub armour_grade: u8,
    pub shield_grade: u8,
    pub engine_grade: u8,
    /// half as much hold again
    pub cargo_ext: bool,
    /// units of hold hidden from customs
    pub hidden: u32,
    /// 0 none .. 3: the chance a second of breaking a missile's lock
    pub ecm: u8,
    pub launcher: bool,
    pub dumb: u32,
    pub seekers: u32,
    /// a rear turret that fires on its own
    pub turret: bool,
}

/// Guns are saved by name.
mod gun_names {
    use super::{Gun, DRIVER, LASER, PLASMA};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(guns: &[Gun], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(guns.iter().map(|g| g.name))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Gun>, D::Error> {
        let names = Vec::<String>::deserialize(d)?;
        Ok(names.iter().filter_map(|n| [LASER, DRIVER, PLASMA].into_iter().find(|g| g.name == n)).collect())
    }
}

impl Kit {
    pub fn stock(c: &Class) -> Kit {
        Kit {
            guns: c.guns.to_vec(),
            armour_grade: 1,
            shield_grade: 1,
            engine_grade: 1,
            cargo_ext: false,
            hidden: 0,
            ecm: 0,
            launcher: false,
            dumb: 0,
            seekers: 0,
            turret: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ship {
    pub id: u32,
    pub class: &'static Class,
    pub kit: Kit,
    pub side: Side,
    /// whose ship it is (the player's ship counts as the Reach's own)
    pub faction: Faction,
    pub callsign: String,
    pub pos: V2,
    pub vel: V2,
    pub heading: f32,
    pub set_speed: f32,
    pub energy: f32,
    pub shields: [f32; 4],
    pub armour: [f32; 4],
    pub hull: f32,
    pub fuel: f32,
    pub burning: bool,
    /// per gun, seconds until it can fire again
    pub cooldown: Vec<f32>,
    /// which gun fires next, so a pair alternates
    pub next_gun: usize,
    /// seconds since the last hit, per facing, for the flash on screen
    pub hit_flash: [f32; 4],
    /// seconds until the rear turret or the launcher can fire again
    pub turret_cd: f32,
    pub launch_cd: f32,
    pub alive: bool,
    /// knocked out: dead in space, drifting, and open to boarding
    pub disabled: bool,
    /// set by a quest that spawned it, to find it again
    pub tag: Option<String>,
    /// something a quest put aboard, found in the hold when boarded
    pub item: Option<String>,
    /// a quest's say on who is aboard (none on an old wreck)
    pub crew: Option<u8>,
}

impl Ship {
    pub fn new(id: u32, class: &'static Class, side: Side, callsign: impl Into<String>, pos: V2, heading: f32) -> Ship {
        Ship {
            id,
            class,
            kit: Kit::stock(class),
            side,
            faction: if side == Side::Pirate { Faction::Pirates } else { Faction::Coop },
            callsign: callsign.into(),
            pos,
            vel: V2::from_heading(heading) * (class.max_speed * 0.5),
            heading,
            set_speed: class.max_speed * 0.5,
            energy: class.energy,
            shields: [class.shield; 4],
            armour: class.armour,
            hull: class.hull,
            fuel: class.fuel,
            burning: false,
            cooldown: vec![0.0; class.guns.len()],
            next_gun: 0,
            hit_flash: [9.0; 4],
            turret_cd: 0.0,
            launch_cd: 0.0,
            alive: true,
            disabled: false,
            tag: None,
            item: None,
            crew: None,
        }
    }

    pub fn guns(&self) -> &[Gun] {
        &self.kit.guns
    }

    pub fn max_shield(&self) -> f32 {
        self.class.shield * (1.0 + 0.4 * (self.kit.shield_grade.max(1) - 1) as f32)
    }

    pub fn max_armour(&self) -> [f32; 4] {
        self.class.armour.map(|a| a * self.kit.armour_grade.max(1) as f32)
    }

    pub fn max_energy(&self) -> f32 {
        self.class.energy * (1.0 + 0.3 * (self.kit.engine_grade.max(1) - 1) as f32)
    }

    fn recharge(&self) -> f32 {
        self.class.recharge * (1.0 + 0.3 * (self.kit.engine_grade.max(1) - 1) as f32)
    }

    pub fn cargo_cap(&self) -> u32 {
        if self.kit.cargo_ext { self.class.cargo * 3 / 2 } else { self.class.cargo }
    }

    /// Refits: new kit, everything topped up to its new limits.
    pub fn refit(&mut self, kit: Kit) {
        let frac: Vec<f32> = (0..4).map(|i| self.armour[i] / self.max_armour()[i]).collect();
        self.kit = kit;
        let max = self.max_armour();
        for i in 0..4 {
            self.armour[i] = max[i] * frac[i];
        }
        self.shields = [self.max_shield(); 4];
        self.energy = self.max_energy();
        self.cooldown = vec![0.0; self.kit.guns.len()];
        self.next_gun = 0;
    }

    pub fn forward(&self) -> V2 {
        V2::from_heading(self.heading)
    }

    pub fn speed(&self) -> f32 {
        self.vel.len()
    }

    /// Hull left, 0..1.
    pub fn hull_frac(&self) -> f32 {
        (self.hull / self.class.hull).clamp(0.0, 1.0)
    }

    /// One tick of flight. Returns true when the afterburner lit this tick.
    pub fn fly(&mut self, it: &Intent, dt: f32) -> bool {
        let c = self.class;
        self.heading += it.turn.clamp(-1.0, 1.0) * c.turn_rate * dt;
        self.heading = turn_to(0.0, self.heading);
        if let Some(s) = it.set_speed {
            self.set_speed = s;
        } else {
            self.set_speed += it.throttle.clamp(-1.0, 1.0) * c.max_speed / 1.5 * dt;
        }
        self.set_speed = self.set_speed.clamp(0.0, c.max_speed);
        let was = self.burning;
        self.burning = it.burner && self.fuel > 0.0;
        let (target, accel) = if self.burning {
            self.fuel = (self.fuel - dt).max(0.0);
            (c.burner_speed, c.accel * 2.0)
        } else {
            // fuel creeps back while the burner rests
            self.fuel = (self.fuel + dt * 0.25).min(c.fuel);
            (self.set_speed, c.accel)
        };
        self.vel = self.vel.approach(self.forward() * target, accel * dt);
        self.pos += self.vel * dt;
        // energy recharges; the shields draw on it
        let (max_e, max_s, regen) = (self.max_energy(), self.max_shield(), c.shield_regen * self.kit.shield_grade.max(1) as f32);
        self.energy = (self.energy + self.recharge() * dt).min(max_e);
        for s in &mut self.shields {
            let want = (max_s - *s).min(regen * dt).min(self.energy);
            if want > 0.0 {
                *s += want;
                self.energy -= want;
            }
        }
        for f in &mut self.hit_flash {
            *f += dt;
        }
        for cd in &mut self.cooldown {
            *cd = (*cd - dt).max(0.0);
        }
        self.turret_cd = (self.turret_cd - dt).max(0.0);
        self.launch_cd = (self.launch_cd - dt).max(0.0);
        self.burning && !was
    }

    /// The next gun ready to fire, if any and if there is energy for it:
    /// guns fire in turn, so a pair alternates.
    pub fn ready_gun(&self) -> Option<usize> {
        let n = self.kit.guns.len();
        (0..n).map(|k| (self.next_gun + k) % n).find(|&g| self.cooldown[g] <= 0.0 && self.energy >= self.kit.guns[g].energy)
    }

    /// Takes a blow of `damage` from the direction `from` (a heading).
    pub fn take_hit(&mut self, damage: f32, from: f32) -> Struck {
        let f = Facing::struck(self.heading, from).index();
        self.hit_flash[f] = 0.0;
        let mut d = damage;
        let absorbed = d.min(self.shields[f]);
        self.shields[f] -= absorbed;
        d -= absorbed;
        if d <= 0.0 {
            return Struck::Shield;
        }
        let into_armour = d.min(self.armour[f]);
        self.armour[f] -= into_armour;
        d -= into_armour;
        if d <= 0.0 {
            return Struck::Armour;
        }
        self.hull -= d;
        if self.hull <= 0.0 {
            self.hull = 0.0;
            self.alive = false;
            Struck::Destroyed
        } else {
            Struck::Hull
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn velocity_settles_on_heading_times_set_speed() {
        let mut s = Ship::new(1, &TIIRA, Side::Player, "you", V2::ZERO, 0.0);
        let it = Intent { set_speed: Some(200.0), ..Default::default() };
        for _ in 0..240 {
            s.fly(&it, 1.0 / 60.0);
        }
        assert!((s.speed() - 200.0).abs() < 0.5);
        assert!(s.vel.y < 0.0, "flying north, up the screen");
    }

    #[test]
    fn a_hard_turn_drifts_before_it_takes() {
        let mut s = Ship::new(1, &TIIRA, Side::Player, "you", V2::ZERO, 0.0);
        s.vel = V2::from_heading(0.0) * 280.0;
        s.heading = FRAC_PI_2;
        let it = Intent { set_speed: Some(280.0), ..Default::default() };
        s.fly(&it, 1.0 / 60.0);
        assert!(s.vel.y < -200.0, "still sliding north just after the turn");
        for _ in 0..180 {
            s.fly(&it, 1.0 / 60.0);
        }
        assert!(s.vel.x > 270.0, "and then flying east");
    }

    #[test]
    fn the_afterburner_spends_fuel_and_goes_faster() {
        let mut s = Ship::new(1, &TIIRA, Side::Player, "you", V2::ZERO, 0.0);
        let it = Intent { burner: true, ..Default::default() };
        let mut lit = false;
        for _ in 0..120 {
            lit |= s.fly(&it, 1.0 / 60.0);
        }
        assert!(lit);
        assert!(s.speed() > TIIRA.max_speed);
        assert!(s.fuel < TIIRA.fuel);
    }

    #[test]
    fn blows_land_on_the_facing_they_come_from() {
        assert_eq!(Facing::struck(0.0, 0.0), Facing::Front);
        assert_eq!(Facing::struck(0.0, PI), Facing::Rear);
        assert_eq!(Facing::struck(0.0, FRAC_PI_2), Facing::Right);
        assert_eq!(Facing::struck(FRAC_PI_2, 0.0), Facing::Left, "facing east, a shot from the north is on the left");
    }

    #[test]
    fn shields_then_armour_then_hull() {
        let mut s = Ship::new(1, &KARPPA, Side::Pirate, "x", V2::ZERO, 0.0);
        assert_eq!(s.take_hit(5.0, 0.0), Struck::Shield);
        assert_eq!(s.take_hit(10.0, 0.0), Struck::Armour);
        assert_eq!(s.shields[0], 0.0);
        assert_eq!(s.take_hit(10.0, 0.0), Struck::Hull);
        assert_eq!(s.shields[1], KARPPA.shield, "the rear never felt it");
        assert_eq!(s.take_hit(100.0, 0.0), Struck::Destroyed);
        assert!(!s.alive);
    }
}
