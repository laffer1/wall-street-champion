//! Player portfolio: cash (integer cents), fractional share holdings, and a
//! margin loan. Buying power = cash + up to 1x equity (2x leverage);
//! interest 0.5%/week on the borrowed amount.

use std::collections::HashMap;

pub const START_CASH_CENTS: i64 = 100_000_00; // $100,000.00
pub const WEEKLY_LIVING_EXPENSES_CENTS: i64 = 1_500_00; // $1,500 / week
pub const GOAL_CENTS: i64 = 1_000_000_00; // $1,000,000
pub const MARGIN_INTEREST_WEEKLY: f64 = 0.005; // 0.5% per week (~26% APR)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Debug)]
pub struct Order {
    pub symbol_idx: usize,
    pub side: Side,
    pub shares: f64,
}

#[derive(Clone, Debug)]
pub struct Portfolio {
    /// Cash in cents; negative means margin debt.
    pub cash_cents: i64,
    pub holdings: HashMap<usize, f64>,
    pub queued: Vec<Order>,
}

#[derive(Clone, Debug)]
pub struct Fill {
    pub symbol_idx: usize,
    pub side: Side,
    pub shares: f64,
    pub price: f64,
}

impl Portfolio {
    pub fn new() -> Self {
        Portfolio {
            cash_cents: START_CASH_CENTS,
            holdings: HashMap::new(),
            queued: Vec::new(),
        }
    }

    pub fn debt_cents(&self) -> i64 {
        if self.cash_cents < 0 { -self.cash_cents } else { 0 }
    }

    pub fn holdings_value(&self, prices: &[f64]) -> f64 {
        self.holdings
            .iter()
            .map(|(&i, &sh)| sh * prices[i])
            .sum::<f64>()
    }

    pub fn net_worth_cents(&self, prices: &[f64]) -> i64 {
        self.cash_cents + (self.holdings_value(prices) * 100.0) as i64
    }

    pub fn equity_cents(&self, prices: &[f64]) -> i64 {
        self.net_worth_cents(prices)
    }

    /// Maximum additional dollars of buying power right now (cash + margin).
    pub fn buying_power_cents(&self, prices: &[f64]) -> i64 {
        let eq = self.equity_cents(prices);
        self.cash_cents + eq.max(0)
    }

    pub fn shares_of(&self, symbol_idx: usize) -> f64 {
        *self.holdings.get(&symbol_idx).unwrap_or(&0.0)
    }

    pub fn queue(&mut self, o: Order) {
        self.queued.push(o);
    }

    /// Execute all queued orders at the given prices. Returns fills and the
    /// margin rule violation, if any.
    pub fn execute(&mut self, prices: &[f64]) -> Result<Vec<Fill>, String> {
        let mut fills = Vec::new();
        // Debt already on the books before this batch. Market moves can push
        // debt above equity on their own; that alone is not an error (the
        // bankruptcy check handles wipeouts) — only *new* borrowing beyond
        // the 2x limit is rejected.
        let baseline_debt = (-self.cash_cents as f64 / 100.0).max(0.0);
        let mut cash = self.cash_cents as f64 / 100.0;
        let mut holdings = self.holdings.clone();
        for o in self.queued.drain(..) {
            let price = prices[o.symbol_idx];
            match o.side {
                Side::Buy => {
                    cash -= o.shares * price;
                    *holdings.entry(o.symbol_idx).or_insert(0.0) += o.shares;
                }
                Side::Sell => {
                    let have = *holdings.get(&o.symbol_idx).unwrap_or(&0.0);
                    let sh = o.shares.min(have);
                    cash += sh * price;
                    let left = have - sh;
                    if left <= 1e-9 {
                        holdings.remove(&o.symbol_idx);
                    } else {
                        holdings.insert(o.symbol_idx, left);
                    }
                    fills.push(Fill { symbol_idx: o.symbol_idx, side: o.side, shares: sh, price });
                    continue;
                }
            }
            fills.push(Fill { symbol_idx: o.symbol_idx, side: o.side, shares: o.shares, price });
        }
        // Margin rule: debt may not exceed equity (2x leverage max), and new
        // orders may not increase debt beyond the pre-existing level.
        let hv: f64 = holdings.iter().map(|(&i, &sh)| sh * prices[i]).sum();
        let equity = cash + hv;
        if -cash > equity + 1e-9 && -cash > baseline_debt + 1e-9 {
            return Err(format!(
                "margin call: debt ${:.2} exceeds equity ${:.2} (max 2x leverage)",
                -cash, equity
            ));
        }
        self.cash_cents = (cash * 100.0).round() as i64;
        self.holdings = holdings;
        Ok(fills)
    }

    /// Weekly charges: living expenses + margin interest. Returns the amounts.
    pub fn weekly_charges(&mut self, prices: &[f64]) -> (i64, i64) {
        let interest = if self.cash_cents < 0 {
            (-self.cash_cents as f64 * MARGIN_INTEREST_WEEKLY).round() as i64
        } else {
            0
        };
        self.cash_cents -= WEEKLY_LIVING_EXPENSES_CENTS + interest;
        let _ = prices;
        (WEEKLY_LIVING_EXPENSES_CENTS, interest)
    }

    /// Credit dividends. Returns total credited in cents.
    pub fn credit_dividends(&mut self, dividends: &[(usize, f64)]) -> i64 {
        let mut total = 0.0;
        for &(i, per_share) in dividends {
            let sh = self.shares_of(i);
            if sh > 0.0 {
                total += sh * per_share;
            }
        }
        let cents = (total * 100.0).round() as i64;
        self.cash_cents += cents;
        cents
    }
}

pub fn fmt_money(cents: i64) -> String {
    let neg = cents < 0;
    let c = cents.abs();
    format!("{}${}.{:02}", if neg { "-" } else { "" }, c / 100, c % 100)
}

pub fn fmt_price(p: f64) -> String {
    if p >= 100.0 {
        format!("${:.2}", p)
    } else if p >= 1.0 {
        format!("${:.3}", p)
    } else {
        format!("${:.4}", p)
    }
}
