//! The Northern Reach: belt regions joined by drive lanes (David, 2026-11-07:
//! regions, not star systems). The Reach is the high-inclination fringe of
//! the main belt; `x` on the sector map runs along the belt, `y` is height
//! above the ecliptic (0 the low belt, 1 the high fringe).
//!
//! Names are placeholders until the Reach's own names are settled. Pallas is
//! a real body and keeps its name.

use super::faction::Faction;
use super::rng::Rng;
use super::v2::{v2, V2};

/// Half the width of a region, metres: where its lane mouths sit, roughly.
pub const REGION_RADIUS: f32 = 14_000.0;
/// How far out from the centre a lane mouth sits.
pub const MOUTH_RANGE: f32 = 11_500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseKind {
    /// a trade hub: a little of everything
    Hub,
    Mine,
    Refinery,
    /// a farm habitat: food and air
    Farm,
    Clinic,
    /// the low belt's customs post at the bottom of the Reach
    Customs,
}

impl BaseKind {
    pub fn name(self) -> &'static str {
        match self {
            BaseKind::Hub => "trade hub",
            BaseKind::Mine => "mining station",
            BaseKind::Refinery => "refinery",
            BaseKind::Farm => "farm habitat",
            BaseKind::Clinic => "clinic",
            BaseKind::Customs => "customs post",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BaseDef {
    pub name: &'static str,
    pub kind: BaseKind,
    /// position in the region, metres
    pub pos: V2,
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct RegionDef {
    pub name: &'static str,
    /// one line for the nav map
    pub note: &'static str,
    /// on the sector map, 0..1 each way
    pub map: V2,
    /// 0 quiet .. 3 pirate country
    pub threat: u32,
    /// who patrols it and runs its bases
    pub owner: Faction,
    /// how many rocks
    pub rocks: u32,
    pub bases: Vec<BaseDef>,
}

pub struct Sector {
    pub regions: Vec<RegionDef>,
    /// undirected lanes, by region index
    pub lanes: Vec<(usize, usize)>,
}

fn base(name: &'static str, kind: BaseKind, x: f32, y: f32, radius: f32) -> BaseDef {
    BaseDef { name, kind, pos: v2(x, y), radius }
}

impl Sector {
    pub fn reach() -> Sector {
        use BaseKind::*;
        use Faction::*;
        let r = |name, note, mx, my, threat, owner, rocks, bases| RegionDef { name, note, map: v2(mx, my), threat, owner, rocks, bases };
        let regions = vec![
            r("Pallas Reach", "the Reach's heart, round Pallas", 0.50, 0.55, 1, Coop, 26, vec![base("Routa Station", Hub, 0.0, 0.0, 260.0)]),
            r("Kide Drift", "ice and rare earths, thin patrols", 0.30, 0.42, 1, Coop, 34, vec![base("Kide Mine", Mine, 2600.0, -1800.0, 200.0)]),
            r("Halla Shoal", "where the Reach's ore is smelted", 0.70, 0.40, 1, Crown, 22, vec![base("Halla Refinery", Refinery, -1500.0, 2200.0, 240.0)]),
            r("Vaskivuori", "a mountain of copper, slowly eaten", 0.18, 0.62, 2, Coop, 40, vec![base("Vaski Works", Mine, 0.0, 3000.0, 220.0)]),
            r("Lumi Field", "greenhouses on the ice", 0.82, 0.62, 1, Coop, 18, vec![base("Lumi Farm", Farm, 2000.0, 1000.0, 230.0)]),
            r("Revontuli Gap", "no stations, no law, good salvage", 0.45, 0.20, 3, Pirates, 30, vec![]),
            r("Sisu Hollow", "the Reach's only hospital", 0.62, 0.78, 1, Traders, 16, vec![base("Sisu Clinic", Clinic, -2200.0, -1200.0, 200.0)]),
            r("Kalmisto", "a field of wrecks; nobody asks", 0.12, 0.30, 3, Pirates, 44, vec![]),
            r("Crown Gate", "the lane down to the low belt", 0.45, 0.92, 0, Inner, 12, vec![base("Gate Customs", Customs, 0.0, 0.0, 280.0)]),
        ];
        let lanes = vec![(0, 1), (0, 2), (0, 6), (0, 8), (1, 3), (1, 5), (1, 7), (2, 4), (2, 5), (3, 7), (4, 6), (6, 8)];
        Sector { regions, lanes }
    }

    pub fn neighbours(&self, r: usize) -> Vec<usize> {
        self.lanes.iter().filter_map(|&(a, b)| if a == r { Some(b) } else if b == r { Some(a) } else { None }).collect()
    }

    /// Where the lane to `to` opens in region `from`: out toward the other
    /// region as the sector map draws it.
    pub fn mouth(&self, from: usize, to: usize) -> V2 {
        let d = self.regions[to].map - self.regions[from].map;
        d.norm() * MOUTH_RANGE
    }

    /// The fewest-lanes route from `a` to `b`, not counting `a`.
    pub fn route(&self, a: usize, b: usize) -> Vec<usize> {
        let n = self.regions.len();
        let mut prev = vec![usize::MAX; n];
        let mut seen = vec![false; n];
        let mut q = std::collections::VecDeque::from([a]);
        seen[a] = true;
        while let Some(r) = q.pop_front() {
            if r == b {
                break;
            }
            for nb in self.neighbours(r) {
                if !seen[nb] {
                    seen[nb] = true;
                    prev[nb] = r;
                    q.push_back(nb);
                }
            }
        }
        let mut path = Vec::new();
        let mut r = b;
        while r != a && prev[r] != usize::MAX {
            path.push(r);
            r = prev[r];
        }
        if r != a {
            return Vec::new();
        }
        path.reverse();
        path
    }

    /// Every base, as (region, index in region).
    pub fn all_bases(&self) -> Vec<(usize, usize)> {
        self.regions.iter().enumerate().flat_map(|(r, d)| (0..d.bases.len()).map(move |b| (r, b))).collect()
    }
}

/// A rock in a region.
#[derive(Clone, Copy, Debug)]
pub struct RockDef {
    pub pos: V2,
    pub radius: f32,
    pub seed: u32,
}

/// The rocks of a region: the same every visit (seeded by the sector seed
/// and the region), clear of bases and lane mouths.
pub fn rocks(seed: u64, region: usize, sector: &Sector) -> Vec<RockDef> {
    let def = &sector.regions[region];
    let mut rng = Rng::new(seed ^ (0x9E37_79B9_7F4A_7C15u64.wrapping_mul(region as u64 + 1)));
    let mut keep_clear: Vec<(V2, f32)> = def.bases.iter().map(|b| (b.pos, b.radius + 1100.0)).collect();
    for nb in sector.neighbours(region) {
        keep_clear.push((sector.mouth(region, nb), 1200.0));
    }
    let mut out: Vec<RockDef> = Vec::new();
    let mut tries = 0;
    while out.len() < def.rocks as usize && tries < 4000 {
        tries += 1;
        let r = if rng.chance(0.15) { rng.range(260.0, 440.0) } else { rng.range(50.0, 200.0) };
        let pos = V2::from_heading(rng.range(-3.14, 3.14)) * rng.range(600.0, REGION_RADIUS * 0.95);
        if keep_clear.iter().any(|&(c, k)| (c - pos).len() < k + r) || out.iter().any(|o| (o.pos - pos).len() < o.radius + r + 250.0) {
            continue;
        }
        out.push(RockDef { pos, radius: r, seed: rng.next_u64() as u32 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_region_is_reachable() {
        let s = Sector::reach();
        for b in 1..s.regions.len() {
            assert!(!s.route(0, b).is_empty(), "{} unreachable", s.regions[b].name);
        }
        assert_eq!(s.route(0, 0), Vec::<usize>::new());
        assert_eq!(s.route(0, 1), vec![1]);
    }

    #[test]
    fn mouths_point_at_their_neighbours() {
        let s = Sector::reach();
        let m = s.mouth(0, 8);
        assert!(m.y > 0.0, "Crown Gate lies below Pallas on the map");
        assert!((m.len() - MOUTH_RANGE).abs() < 1.0);
    }

    #[test]
    fn rocks_keep_clear_of_bases_and_mouths() {
        let s = Sector::reach();
        for r in 0..s.regions.len() {
            let rs = rocks(1, r, &s);
            for b in &s.regions[r].bases {
                assert!(rs.iter().all(|k| (k.pos - b.pos).len() > b.radius + k.radius + 1000.0));
            }
            assert_eq!(rocks(1, r, &s).len(), rs.len(), "the same every visit");
        }
    }
}
