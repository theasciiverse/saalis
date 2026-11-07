//! Trade: commodities, and a market at every base whose stocks move with
//! your trades and drift back to normal by the hour.
//!
//! A base produces some goods (cheap there, plenty in stock) and consumes
//! others (dear, scarce). The price follows the stock: buying a base out
//! raises its price, flooding it lowers it. The sector index is the average
//! price over every base, to compare against.

use super::sector::BaseKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct Commodity {
    pub name: &'static str,
    /// credits per unit, at no bias and normal stock
    pub price: f32,
}

pub const GOODS: [Commodity; 10] = [
    Commodity { name: "Water ice", price: 18.0 },
    Commodity { name: "Volatiles", price: 34.0 },
    Commodity { name: "Iron-nickel", price: 42.0 },
    Commodity { name: "Rare earths", price: 160.0 },
    Commodity { name: "Hydroponic food", price: 55.0 },
    Commodity { name: "Breathing air", price: 40.0 },
    Commodity { name: "Medicine", price: 210.0 },
    Commodity { name: "Drive parts", price: 260.0 },
    Commodity { name: "Machinery", price: 140.0 },
    Commodity { name: "Luxuries", price: 320.0 },
];
pub const N: usize = GOODS.len();

/// What a kind of base makes (+) and needs (-), per good, -1..1.
pub fn bias(kind: BaseKind) -> [f32; N] {
    //                       ice   vol   iron  rare  food  air   med   drive mach  lux
    match kind {
        BaseKind::Hub => [0.0, 0.0, -0.2, 0.0, -0.3, -0.2, -0.2, 0.0, 0.2, 0.3],
        BaseKind::Mine => [0.8, 0.3, 0.9, 0.7, -0.8, -0.7, -0.5, -0.4, -0.8, -0.3],
        BaseKind::Refinery => [-0.6, 0.8, -0.9, -0.5, -0.5, -0.4, -0.3, 0.8, 0.7, -0.2],
        BaseKind::Farm => [-0.9, -0.4, 0.0, 0.0, 0.9, 0.9, -0.4, -0.3, -0.6, 0.0],
        BaseKind::Clinic => [-0.3, -0.2, 0.0, -0.6, -0.4, -0.3, 0.9, 0.0, -0.4, -0.5],
        BaseKind::Customs => [0.0, 0.0, 0.0, -0.3, 0.0, 0.0, 0.0, -0.4, -0.3, 0.7],
    }
}

/// One base's market.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Market {
    pub stock: [f32; N],
    #[serde(skip, default = "zero")]
    pub bias: [f32; N],
}

/// Two markets are the same when their stocks are; the bias comes from the
/// kind of base and is not saved.
impl PartialEq for Market {
    fn eq(&self, o: &Market) -> bool {
        self.stock == o.stock
    }
}

fn zero() -> [f32; N] {
    [0.0; N]
}

/// Normal stock for a good at a given bias.
fn normal(b: f32) -> f32 {
    100.0 * (1.0 + b)
}

impl Market {
    pub fn new(kind: BaseKind) -> Market {
        let bias = bias(kind);
        Market { stock: bias.map(normal), bias }
    }

    /// Restores the bias after loading a saved market.
    pub fn with_kind(mut self, kind: BaseKind) -> Market {
        self.bias = bias(kind);
        self
    }

    /// What the base sells a unit for.
    pub fn buy_price(&self, g: usize) -> u32 {
        let b = self.bias[g];
        let scarcity = ((normal(b) - self.stock[g]) / 100.0).clamp(-0.6, 1.2);
        (GOODS[g].price * (1.0 - 0.4 * b) * (1.0 + 0.35 * scarcity)).round().max(1.0) as u32
    }

    /// What the base pays for a unit.
    pub fn sell_price(&self, g: usize) -> u32 {
        ((self.buy_price(g) as f32) * 0.88).round().max(1.0) as u32
    }

    /// Whether the base has one to sell.
    pub fn has(&self, g: usize) -> bool {
        self.stock[g] >= 1.0
    }

    pub fn bought(&mut self, g: usize, n: u32) {
        self.stock[g] = (self.stock[g] - n as f32).max(0.0);
    }

    pub fn sold(&mut self, g: usize, n: u32) {
        self.stock[g] += n as f32;
    }

    /// Stocks drift back toward normal: about a tenth of the way an hour.
    pub fn drift(&mut self, hours: f32) {
        let k = 1.0 - 0.9f32.powf(hours);
        for g in 0..N {
            let want = normal(self.bias[g]);
            self.stock[g] += (want - self.stock[g]) * k;
        }
    }
}

/// The sector index: the mean buying price over every market.
pub fn index(markets: &[Market], g: usize) -> u32 {
    if markets.is_empty() {
        return GOODS[g].price as u32;
    }
    (markets.iter().map(|m| m.buy_price(g) as f32).sum::<f32>() / markets.len() as f32).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producers_sell_cheap_and_consumers_pay_well() {
        let mine = Market::new(BaseKind::Mine);
        let farm = Market::new(BaseKind::Farm);
        let ice = 0;
        assert!(mine.buy_price(ice) < farm.sell_price(ice), "ice from the mine to the farm pays");
        let food = 4;
        assert!(farm.buy_price(food) < mine.sell_price(food), "and food back pays too");
    }

    #[test]
    fn buying_raises_the_price_and_time_brings_it_back() {
        let mut m = Market::new(BaseKind::Mine);
        let p0 = m.buy_price(2);
        m.bought(2, 120);
        let p1 = m.buy_price(2);
        assert!(p1 > p0);
        m.drift(48.0);
        assert!(m.buy_price(2) <= p0 + 1, "two days later it has recovered");
    }

    #[test]
    fn selling_always_pays_less_than_buying_at_one_base() {
        for kind in [BaseKind::Hub, BaseKind::Mine, BaseKind::Refinery, BaseKind::Farm, BaseKind::Clinic, BaseKind::Customs] {
            let m = Market::new(kind);
            for g in 0..N {
                assert!(m.sell_price(g) < m.buy_price(g), "no free money at one counter");
            }
        }
    }
}
