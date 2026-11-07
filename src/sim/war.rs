//! The living belt: who holds each region, how strongly, and the fleets on
//! the lanes (Ascii Sector's persistent fleets that take systems and go
//! home). It runs off screen, an hour at a time.
//!
//! Claim-jumper fleets gather in the regions they hold and strike at weak
//! neighbours. The regions' rightful owners send fleets to take back what
//! was lost. A fleet that beats the garrison takes the region. The player
//! matters: every claim-jumper the player kills in a contested region
//! weakens their garrison or the fleet bound for it.

use super::faction::Faction;
use super::rng::Rng;
use super::sector::Sector;
use serde::{Deserialize, Serialize};

/// Game hours a fleet spends on a lane.
pub const FLEET_HOURS: f64 = 6.0;
const MAX_FLEETS: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fleet {
    pub faction: Faction,
    pub from: usize,
    pub to: usize,
    pub strength: f32,
    /// the game hour it arrives
    pub eta: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct War {
    pub control: Vec<Faction>,
    /// the holder's strength in each region, 0..100
    pub garrison: Vec<f32>,
    pub fleets: Vec<Fleet>,
}

/// What happened, for the comms and the log.
#[derive(Clone, Debug, PartialEq)]
pub enum News {
    Sighted { faction: Faction, to: usize },
    Taken { region: usize, by: Faction, from: Faction },
    Held { region: usize, against: Faction },
    /// a fleet arrived where the player is: a battle to spawn
    Battle { region: usize },
}

fn cap(f: Faction) -> f32 {
    match f {
        Faction::Inner => 100.0,
        Faction::Pirates => 60.0,
        _ => 80.0,
    }
}

impl War {
    pub fn new(s: &Sector) -> War {
        let control: Vec<Faction> = s.regions.iter().map(|r| r.owner).collect();
        let garrison = control.iter().map(|&f| cap(f) * 0.85).collect();
        War { control, garrison, fleets: Vec::new() }
    }

    /// One game hour of the war. `here` is the player's region.
    pub fn hour(&mut self, s: &Sector, rng: &mut Rng, clock: f64, here: usize) -> Vec<News> {
        let mut news = Vec::new();
        for r in 0..s.regions.len() {
            self.garrison[r] = (self.garrison[r] + 0.6).min(cap(self.control[r]));
        }
        // claim-jumpers strike out from what they hold
        for r in 0..s.regions.len() {
            if self.fleets.len() >= MAX_FLEETS || self.control[r] != Faction::Pirates || !rng.chance(0.03) {
                continue;
            }
            let weakest = s
                .neighbours(r)
                .into_iter()
                .filter(|&n| self.control[n] != Faction::Pirates && s.regions[n].threat > 0)
                .min_by(|&a, &b| self.garrison[a].total_cmp(&self.garrison[b]));
            if let Some(to) = weakest {
                let strength = rng.range(35.0, 75.0);
                self.fleets.push(Fleet { faction: Faction::Pirates, from: r, to, strength, eta: clock + FLEET_HOURS });
                news.push(News::Sighted { faction: Faction::Pirates, to });
            }
        }
        // owners take back what they lost
        for r in 0..s.regions.len() {
            let owner = s.regions[r].owner;
            if self.fleets.len() >= MAX_FLEETS || self.control[r] == owner || self.fleets.iter().any(|f| f.to == r && f.faction == owner) || !rng.chance(0.06) {
                continue;
            }
            let base = s.neighbours(r).into_iter().find(|&n| self.control[n] == owner).unwrap_or(r);
            let strength = rng.range(50.0, 90.0);
            self.fleets.push(Fleet { faction: owner, from: base, to: r, strength, eta: clock + FLEET_HOURS });
            news.push(News::Sighted { faction: owner, to: r });
        }
        // arrivals and battles
        let (due, rest): (Vec<Fleet>, Vec<Fleet>) = std::mem::take(&mut self.fleets).into_iter().partition(|f| clock >= f.eta);
        self.fleets = rest;
        for f in due {
            let r = f.to;
            if r == here {
                news.push(News::Battle { region: r });
            }
            let holder = self.control[r];
            if !f.faction.at_war(holder) {
                self.garrison[r] = (self.garrison[r] + f.strength * 0.5).min(cap(holder));
                continue;
            }
            if f.strength > self.garrison[r] {
                self.garrison[r] = ((f.strength - self.garrison[r] * 0.5).max(10.0)).min(cap(f.faction));
                self.control[r] = f.faction;
                news.push(News::Taken { region: r, by: f.faction, from: holder });
            } else {
                self.garrison[r] = (self.garrison[r] - f.strength * 0.6).max(5.0);
                news.push(News::Held { region: r, against: f.faction });
            }
        }
        news
    }

    /// A claim-jumper killed by the player in region `r`.
    pub fn pirate_down(&mut self, r: usize) {
        if self.control[r] == Faction::Pirates {
            self.garrison[r] = (self.garrison[r] - 3.0).max(0.0);
        }
        for f in self.fleets.iter_mut().filter(|f| f.to == r && f.faction == Faction::Pirates) {
            f.strength = (f.strength - 4.0).max(0.0);
        }
    }

    /// How dangerous region `r` is now: its nature, worse under pirates.
    pub fn threat(&self, s: &Sector, r: usize) -> u32 {
        let base = s.regions[r].threat;
        match (s.regions[r].owner == Faction::Pirates, self.control[r] == Faction::Pirates) {
            (false, true) => (base + 2).min(3),
            (true, false) => base.saturating_sub(1),
            _ => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_belt_changes_hands_over_time_and_owners_strike_back() {
        let s = Sector::reach();
        let mut w = War::new(&s);
        let mut rng = Rng::new(9);
        let mut taken = 0;
        let mut retaken = 0;
        for h in 0..24 * 60 {
            for n in w.hour(&s, &mut rng, h as f64, 0) {
                if let News::Taken { by, .. } = n {
                    if by == Faction::Pirates {
                        taken += 1;
                    } else {
                        retaken += 1;
                    }
                }
            }
        }
        assert!(taken > 0, "the claim-jumpers took something in two months");
        assert!(retaken > 0, "and lost something back");
        assert!(w.fleets.len() <= MAX_FLEETS);
    }

    #[test]
    fn killing_pirates_weakens_their_hold() {
        let s = Sector::reach();
        let mut w = War::new(&s);
        let r = s.regions.iter().position(|x| x.owner == Faction::Pirates).unwrap();
        let g = w.garrison[r];
        w.pirate_down(r);
        assert!(w.garrison[r] < g);
    }
}
