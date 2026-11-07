//! Missions, as in Ascii Sector: guild boards at the bases and fixers in
//! the bars, every job with a deadline. Two guilds (placeholder names): the
//! Free Traders' Guild hands out cargo and courier runs, the Reach Watch
//! (the cooperatives' militia) bounties and patrols. Joining a guild costs
//! a fee and opens its whole board; non-members see one open contract
//! each. Every job done raises your rating with that guild, and pay rises
//! with it. A fixer offers one riskier, better-paid job, no membership
//! asked.

use super::econ;
use super::faction::Faction;
use super::rng::Rng;
use super::sector::Sector;
use serde::{Deserialize, Serialize};

pub const MAX_ACTIVE: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Guild {
    Traders,
    Watch,
}

impl Guild {
    pub const ALL: [Guild; 2] = [Guild::Traders, Guild::Watch];

    pub fn name(self) -> &'static str {
        match self {
            Guild::Traders => "Free Traders' Guild",
            Guild::Watch => "Reach Watch",
        }
    }

    pub fn faction(self) -> Faction {
        match self {
            Guild::Traders => Faction::Traders,
            Guild::Watch => Faction::Coop,
        }
    }

    pub fn fee(self) -> u32 {
        match self {
            Guild::Traders => 300,
            Guild::Watch => 500,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Task {
    /// carry `units` of `good` to a base
    Cargo { good: usize, units: u32, region: usize, base: usize },
    /// carry a sealed package to a base
    Courier { region: usize, base: usize },
    /// destroy `need` claim-jumpers in a region
    Bounty { region: usize, need: u32, done: u32 },
    /// fly to every lane mouth of a region
    Patrol { region: usize, mouths: Vec<usize>, seen: Vec<bool> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Issuer {
    Guild(Guild),
    Fixer(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mission {
    pub id: u32,
    pub issuer: Issuer,
    pub task: Task,
    pub pay: u32,
    /// game hour it must be done by
    pub deadline: f64,
}

impl Mission {
    /// Where it sends you.
    pub fn region(&self) -> usize {
        match self.task {
            Task::Cargo { region, .. } | Task::Courier { region, .. } | Task::Bounty { region, .. } | Task::Patrol { region, .. } => region,
        }
    }

    /// Hold space it takes.
    pub fn units(&self) -> u32 {
        match self.task {
            Task::Cargo { units, .. } => units,
            _ => 0,
        }
    }

    /// One line describing it.
    pub fn describe(&self, s: &Sector) -> String {
        let region = |r: usize| s.regions[r].name;
        let base = |r: usize, b: usize| s.regions[r].bases[b].name;
        match &self.task {
            Task::Cargo { good, units, region, base: b } => format!("{units} {} to {}", econ::GOODS[*good].name.to_lowercase(), base(*region, *b)),
            Task::Courier { region, base: b } => format!("a sealed package to {}", base(*region, *b)),
            Task::Bounty { region: r, need, done } => format!("{} claim-jumpers in {} ({done} so far)", need, region(*r)),
            Task::Patrol { region: r, seen, .. } => format!("patrol {}'s lanes ({}/{})", region(*r), seen.iter().filter(|x| **x).count(), seen.len()),
        }
    }

    pub fn issuer_name(&self) -> String {
        match &self.issuer {
            Issuer::Guild(g) => g.name().to_string(),
            Issuer::Fixer(n) => n.clone(),
        }
    }
}

/// Hours left, as words for the board.
pub fn time_left(deadline: f64, now: f64) -> String {
    let h = (deadline - now).max(0.0);
    if h >= 48.0 { format!("{:.0} days", h / 24.0) } else { format!("{:.0} h", h) }
}

/// A fixer's name and opening line (placeholders).
const FIXERS: [(&str, &str); 6] = [
    ("Aino Lepistö", "You look like someone who doesn't ask why. Good."),
    ("Old Hartikainen", "Sit. The coffee's terrible, the job isn't."),
    ("Vesa \"Kide\" Mäki", "Quick work, quiet work, cash on landing."),
    ("Ruut Saarinen", "My client pays double for pilots who can count to deadline."),
    ("Kaarlo", "Don't use my name out there. Use the money."),
    ("Mirja Vainio", "Somebody has to go where the patrols won't."),
];

pub struct Board {
    pub offers: Vec<Mission>,
    /// the fixer in the bar today, if any: name, line, the job
    pub fixer: Option<(String, String, Mission)>,
}

/// What the boards and the bar offer at base `base` of region `region`
/// today. The same all day: seeded by the base and the day.
#[allow(clippy::too_many_arguments)]
pub fn board(seed: u64, s: &Sector, region: usize, base: usize, clock: f64, members: [bool; 2], rating: [u32; 2], next_id: &mut u32) -> Board {
    let day = (clock / 24.0).floor() as u64;
    let mut rng = Rng::new(seed ^ (day.wrapping_mul(0x9E37_79B9) + (region as u64) * 131 + base as u64 * 7 + 1));
    let bases = s.all_bases();
    let others: Vec<(usize, usize)> = bases.iter().copied().filter(|&(r, b)| (r, b) != (region, base)).collect();
    let hops = |to: usize| s.route(region, to).len() as f64;
    let mut id = || {
        *next_id += 1;
        *next_id
    };
    let mut offers = Vec::new();
    let boost = |g: Guild| 1.0 + 0.1 * rating[g.index()].min(10) as f32;
    // the traders' board
    let n = if members[0] { 3 } else { 1 };
    for _ in 0..n {
        let (r, b) = others[rng.below(others.len() as u32) as usize];
        let h = hops(r);
        let task = if rng.chance(0.6) {
            let good = rng.below(econ::N as u32) as usize;
            let units = 5 + rng.below(16);
            Task::Cargo { good, units, region: r, base: b }
        } else {
            Task::Courier { region: r, base: b }
        };
        let units = if let Task::Cargo { units, .. } = task { units } else { 0 };
        let pay = ((200.0 + h as f32 * 260.0 + units as f32 * 18.0) * boost(Guild::Traders)).round() as u32;
        offers.push(Mission { id: id(), issuer: Issuer::Guild(Guild::Traders), task, pay, deadline: clock + 12.0 + h * 9.0 + rng.range(0.0, 12.0) as f64 });
    }
    // the Watch's board: bounties where there is trouble, patrols at home
    let n = if members[1] { 2 } else { 1 };
    let troubled: Vec<usize> = (0..s.regions.len()).filter(|&r| s.regions[r].threat > 0).collect();
    let home: Vec<usize> = (0..s.regions.len()).filter(|&r| s.regions[r].owner == Faction::Coop).collect();
    for _ in 0..n {
        if rng.chance(0.55) && !troubled.is_empty() {
            let r = troubled[rng.below(troubled.len() as u32) as usize];
            let need = 2 + rng.below(3);
            let threat = s.regions[r].threat as f32;
            let pay = ((300.0 + need as f32 * 220.0 * threat) * boost(Guild::Watch)).round() as u32;
            offers.push(Mission { id: id(), issuer: Issuer::Guild(Guild::Watch), task: Task::Bounty { region: r, need, done: 0 }, pay, deadline: clock + 24.0 + hops(r) * 9.0 });
        } else if !home.is_empty() {
            let r = home[rng.below(home.len() as u32) as usize];
            let mouths = s.neighbours(r);
            let seen = vec![false; mouths.len()];
            let pay = ((250.0 + mouths.len() as f32 * 120.0 + hops(r) as f32 * 150.0) * boost(Guild::Watch)).round() as u32;
            offers.push(Mission { id: id(), issuer: Issuer::Guild(Guild::Watch), task: Task::Patrol { region: r, mouths, seen }, pay, deadline: clock + 18.0 + hops(r) * 9.0 });
        }
    }
    // the bar: a fixer some days, with something riskier
    let fixer = rng.chance(0.6).then(|| {
        let (name, line) = FIXERS[rng.below(FIXERS.len() as u32) as usize];
        let wild: Vec<usize> = (0..s.regions.len()).filter(|&r| s.regions[r].threat >= 2).collect();
        let job = if rng.chance(0.5) && !wild.is_empty() {
            let r = wild[rng.below(wild.len() as u32) as usize];
            let need = 3 + rng.below(3);
            Mission { id: id(), issuer: Issuer::Fixer(name.into()), task: Task::Bounty { region: r, need, done: 0 }, pay: 900 + need * 450, deadline: clock + 20.0 + hops(r) * 8.0 }
        } else {
            let (r, b) = others[rng.below(others.len() as u32) as usize];
            let h = hops(r);
            // tight: barely time to fly it
            Mission { id: id(), issuer: Issuer::Fixer(name.into()), task: Task::Courier { region: r, base: b }, pay: (700.0 + h as f32 * 520.0) as u32, deadline: clock + 5.0 + h * 5.5 }
        };
        (name.to_string(), line.to_string(), job)
    });
    Board { offers, fixer }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_see_more_and_the_board_holds_all_day() {
        let s = Sector::reach();
        let mut id = 0;
        let a = board(1, &s, 0, 0, 10.0, [false; 2], [0; 2], &mut id);
        let b = board(1, &s, 0, 0, 10.0, [true; 2], [0; 2], &mut id);
        assert_eq!(a.offers.len(), 2);
        assert_eq!(b.offers.len(), 5);
        let again = board(1, &s, 0, 0, 20.0, [false; 2], [0; 2], &mut id);
        assert_eq!(again.offers[0].task, a.offers[0].task, "the same day, the same board");
    }

    #[test]
    fn rating_raises_pay() {
        let s = Sector::reach();
        let mut id = 0;
        let a = board(3, &s, 0, 0, 10.0, [true; 2], [0; 2], &mut id);
        let b = board(3, &s, 0, 0, 10.0, [true; 2], [10; 2], &mut id);
        assert!(b.offers[0].pay > a.offers[0].pay);
    }

    #[test]
    fn deadlines_leave_time_to_fly() {
        let s = Sector::reach();
        let mut id = 0;
        for day in 0..20 {
            let bd = board(5, &s, 0, 0, day as f64 * 24.0, [true; 2], [0; 2], &mut id);
            for m in bd.offers {
                let lanes = s.route(0, m.region()).len() as f64;
                assert!(m.deadline - day as f64 * 24.0 > lanes * 4.0, "{m:?}");
            }
        }
    }
}
