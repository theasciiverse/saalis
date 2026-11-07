//! The shipyard and the outfitters: hulls, guns, grades of armour, shields
//! and engine, a bigger hold, ECM, a missile launcher and its missiles, a
//! rear turret, and (under the counter, at black markets) hidden
//! compartments. Prices are placeholders, to tune by play.

use super::ship::{self, Kit, Ship};
use super::{Phase, World, PLAYER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Hull(usize),
    Guns(usize),
    Armour,
    Shield,
    Engine,
    CargoExt,
    Ecm,
    Launcher,
    Dumb,
    Seekers,
    Turret,
    Hidden,
}

/// One line in the shipyard: what, its price now, whether it can be had,
/// and a note.
pub struct Offer {
    pub item: Item,
    pub name: String,
    pub price: u32,
    pub ok: bool,
    pub note: String,
}

const RACK: u32 = 8;

fn grade_price(base: u32, grade: u8) -> Option<u32> {
    match grade {
        1 => Some(base),
        2 => Some(base * 5 / 2),
        _ => None,
    }
}

pub fn trade_in(s: &Ship) -> u32 {
    ship::PLAYER_HULLS.iter().find(|h| std::ptr::eq(h.0, s.class)).map_or(0, |h| h.1 * 6 / 10)
}

impl World {
    /// What the base the player is docked at sells.
    pub fn shop(&self) -> Vec<Offer> {
        let Phase::Docked(b) = self.phase else { return Vec::new() };
        let base = &self.bases[b];
        let Some(p) = self.ships.iter().find(|s| s.id == PLAYER) else { return Vec::new() };
        let k = &p.kit;
        let (launcher_mount, turret_mount) = ship::mounts(p.class);
        let mut v = Vec::new();
        let mut add = |item, name: String, price: Option<u32>, ok: bool, note: &str| {
            v.push(Offer { item, name, price: price.unwrap_or(0), ok: ok && price.is_some(), note: if price.is_none() { "fitted, best grade".into() } else { note.to_string() } });
        };
        if base.kind.shipyard() {
            for (i, (c, price)) in ship::PLAYER_HULLS.iter().enumerate() {
                let own = std::ptr::eq(*c, p.class);
                let cost = price.saturating_sub(trade_in(p));
                let note = if own { "your hull".to_string() } else { format!("{} hold, {} guns; trade-in counted", c.cargo, c.guns.len()) };
                add(Item::Hull(i), format!("{} {}", c.name, c.role), (!own).then_some(cost), !own, &note);
            }
        }
        if base.kind.outfitter() {
            let mounts = k.guns.len() as u32;
            for (i, (g, each)) in ship::SHOP_GUNS.iter().enumerate() {
                let fitted = k.guns.iter().all(|x| x.name == g.name);
                add(Item::Guns(i), format!("{}s on all {mounts} mounts", g.name), (!fitted).then_some(each * mounts), true, &format!("{:.0} damage, {:.0} m", g.damage, g.range));
            }
            add(Item::Armour, format!("armour plating, grade {}", k.armour_grade + 1), grade_price(1800, k.armour_grade), true, "more armour on every facing");
            add(Item::Shield, format!("shield generator, grade {}", k.shield_grade + 1), grade_price(2000, k.shield_grade), true, "stronger, faster shields");
            add(Item::Engine, format!("power plant, grade {}", k.engine_grade + 1), grade_price(1600, k.engine_grade), true, "more energy, faster recharge");
            add(Item::CargoExt, "cargo expansion".into(), (!k.cargo_ext).then_some(2500), true, "half as much hold again");
            add(Item::Ecm, format!("ECM, grade {}", k.ecm + 1), (k.ecm < 3).then_some(1200 * (k.ecm as u32 + 1)), true, "a chance a second to break a missile lock");
            if launcher_mount {
                add(Item::Launcher, "missile launcher".into(), (!k.launcher).then_some(1500), true, "fires dumbfire and seeking missiles");
                let room = k.launcher && k.dumb + k.seekers < RACK;
                add(Item::Dumb, format!("dumbfire missile ({} aboard)", k.dumb), Some(60), room, "flies straight");
                add(Item::Seekers, format!("seeking missile ({} aboard)", k.seekers), Some(220), room, "homes on a locked target");
            }
            if turret_mount {
                add(Item::Turret, "rear turret".into(), (!k.turret).then_some(3500), true, "fires on its own at what follows you");
            }
        }
        if base.black_market {
            let cap = p.cargo_cap() / 4;
            add(Item::Hidden, format!("hidden compartment, 5 units ({} now)", k.hidden), Some(400), k.hidden + 5 <= cap, "customs won't see what's in it, mostly");
        }
        v
    }

    /// Buys an item; says why not if it can't.
    pub fn buy_item(&mut self, item: Item) -> Result<String, String> {
        let Some(o) = self.shop().into_iter().find(|o| o.item == item) else { return Err("Not sold here.".into()) };
        if !o.ok {
            return Err(if o.price == 0 { "You have that already.".into() } else { "That won't fit.".into() });
        }
        if self.credits < o.price {
            return Err(format!("That's {} cr.", o.price));
        }
        let cargo = self.cargo_used();
        let Some(p) = self.ships.iter_mut().find(|s| s.id == PLAYER) else { return Err("No ship.".into()) };
        let mut kit = p.kit.clone();
        match item {
            Item::Hull(i) => {
                let c = ship::PLAYER_HULLS[i].0;
                if cargo > c.cargo {
                    return Err(format!("The {} holds only {}. Sell some cargo first.", c.name, c.cargo));
                }
                p.class = c;
                p.refit(Kit::stock(c));
                p.armour = p.max_armour();
                p.hull = c.hull;
                p.fuel = c.fuel;
                self.credits -= o.price;
                log::info!("bought hull {}", c.name);
                return Ok(format!("The {} is yours. Your old fittings went with the old hull.", c.name));
            }
            Item::Guns(i) => {
                let g = ship::SHOP_GUNS[i].0;
                kit.guns = vec![g; kit.guns.len()];
            }
            Item::Armour => kit.armour_grade += 1,
            Item::Shield => kit.shield_grade += 1,
            Item::Engine => kit.engine_grade += 1,
            Item::CargoExt => kit.cargo_ext = true,
            Item::Ecm => kit.ecm += 1,
            Item::Launcher => kit.launcher = true,
            Item::Dumb => kit.dumb += 1,
            Item::Seekers => kit.seekers += 1,
            Item::Turret => kit.turret = true,
            Item::Hidden => kit.hidden += 5,
        }
        // new armour plating arrives whole
        let armour_up = item == Item::Armour;
        p.refit(kit);
        if armour_up {
            p.armour = p.max_armour();
        }
        self.credits -= o.price;
        log::info!("bought {:?} for {}", item, o.price);
        Ok(format!("Fitted: {}.", o.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routa_sells_hulls_and_fittings() {
        let w = World::new(2);
        let shop = w.shop();
        assert!(shop.iter().any(|o| matches!(o.item, Item::Hull(_))));
        assert!(shop.iter().any(|o| o.item == Item::Armour));
        assert!(!shop.iter().any(|o| o.item == Item::Hidden), "no black market at Routa");
    }

    #[test]
    fn grades_raise_the_ship_and_cost_credits() {
        let mut w = World::new(2);
        w.credits = 100_000;
        let a0 = w.ships[0].max_armour()[0];
        w.buy_item(Item::Armour).unwrap();
        assert!(w.ships[0].max_armour()[0] > a0);
        assert_eq!(w.ships[0].armour[0], w.ships[0].max_armour()[0], "new plating arrives whole");
        w.buy_item(Item::Armour).unwrap();
        assert!(w.buy_item(Item::Armour).is_err(), "grade 3 is the best");
        w.buy_item(Item::CargoExt).unwrap();
        assert_eq!(w.cargo_room(), 45);
    }

    #[test]
    fn a_new_hull_needs_room_for_the_cargo() {
        let mut w = World::new(2);
        w.credits = 100_000;
        let haukka = ship::PLAYER_HULLS.iter().position(|h| h.0.name == "Haukka").unwrap();
        w.buy(0, 30);
        assert!(w.buy_item(Item::Hull(haukka)).is_err());
        w.sell(0, 30);
        w.buy_item(Item::Hull(haukka)).unwrap();
        assert_eq!(w.ships[0].class.name, "Haukka");
        assert_eq!(w.ships[0].guns().len(), 4);
    }
}
