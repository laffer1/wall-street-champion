//! Market simulation. Weekly ticks over a 26-week game.
//!
//! Model (parameters anchored to real market history):
//! - Markov regime for the broad market: bull / sideways / bear, with
//!   weekly drift/vol roughly matching long-run S&P 500 behavior
//!   (~16% annualized vol => ~2.2% weekly; bull weeks ~+0.4%, bear ~-0.5%).
//! - One factor per sector plus a market factor: r_i = beta*r_mkt
//!   + sector_beta*r_sec + idiosyncratic.
//! - Volatility clustering: last week's |return| scales this week's vol.
//! - Quarterly earnings (weeks 6/13/19/26) with EPS-surprise jumps of
//!   +/-5..15% (real earnings moves are typically in that band).
//! - Quarterly dividends for yield payers; price drops by the dividend on
//!   the ex week.
//! - Crypto: high vol, positive momentum (bursts), event-driven jumps.

use crate::rng::Rng;
use super::instruments::{all_instruments, is_earnings_week, Kind, Sector};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Regime {
    Bull,
    Sideways,
    Bear,
}

impl Regime {
    fn drift(self) -> f64 {
        match self {
            Regime::Bull => 0.004,
            Regime::Sideways => 0.0008,
            Regime::Bear => -0.005,
        }
    }
    fn vol(self) -> f64 {
        match self {
            Regime::Bull => 0.022,
            Regime::Sideways => 0.017,
            Regime::Bear => 0.034,
        }
    }
    /// Row-stochastic transition matrix, indexed [from][to] as
    /// [Bull, Sideways, Bear].
    fn transition(self, rng: &mut Rng) -> Regime {
        let p = match self {
            Regime::Bull => [0.60, 0.30, 0.10],
            Regime::Sideways => [0.25, 0.50, 0.25],
            Regime::Bear => [0.15, 0.35, 0.50],
        };
        let x = rng.f64();
        if x < p[0] {
            Regime::Bull
        } else if x < p[0] + p[1] {
            Regime::Sideways
        } else {
            Regime::Bear
        }
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub category: EventCategory,
    /// Instrument affected, if company/crypto specific.
    pub symbol_idx: Option<usize>,
    /// Log-return impulse applied to the affected instrument(s) this week.
    pub impulse: f64,
    /// Short factual string used to build headlines.
    pub facts: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventCategory {
    Macro,
    Sector,
    Company,
    Earnings,
    Dividend,
    Crypto,
}

pub struct InstrumentState {
    pub price: f64,
    pub prev_ret: f64,
    pub history: Vec<f64>,
    /// Remaining event impulse to apply (decays weekly).
    pub pending_impulse: f64,
}

pub struct Market {
    pub defs: Vec<super::instruments::InstrumentDef>,
    pub states: Vec<InstrumentState>,
    pub index: f64,
    pub index_history: Vec<f64>,
    pub regime: Regime,
}

/// What is going to happen when the week advances (rolled ahead of time so
/// the news can honestly hint at it).
pub struct PendingWeek {
    pub target_week: u32,
    pub regime: Regime,
    pub index_ret: f64,
    /// Total log return per instrument for the coming week.
    pub rets: Vec<f64>,
    pub events: Vec<Event>,
    /// (instrument, dividend per share) paid on the coming week's ex-date.
    pub dividends: Vec<(usize, f64)>,
    /// (instrument, EPS surprise fraction) for earnings weeks.
    pub earnings: Vec<(usize, f64)>,
}

impl Market {
    pub fn new() -> Self {
        let defs = all_instruments();
        let states = defs
            .iter()
            .map(|d| InstrumentState {
                price: d.start_price,
                prev_ret: 0.0,
                history: vec![d.start_price],
                pending_impulse: 0.0,
            })
            .collect();
        Market {
            defs,
            states,
            index: 1000.0,
            index_history: vec![1000.0],
            regime: Regime::Sideways,
        }
    }

    pub fn symbol_idx(&self, symbol: &str) -> Option<usize> {
        self.defs.iter().position(|d| d.symbol == symbol)
    }

    /// Backfill a few synthetic weeks so charts look alive from week 1.
    pub fn backfill(&mut self, rng: &mut Rng) {
        for _ in 0..8 {
            let r = self.regime.drift() * 0.5 + self.regime.vol() * 0.5 * rng.normal();
            self.index *= r.exp();
            self.index_history.push(self.index);
            for (i, st) in self.states.iter_mut().enumerate() {
                let d = &self.defs[i];
                let rr = d.beta * r + d.weekly_vol * rng.normal();
                st.price = (st.price * rr.exp()).max(0.05);
                st.history.push(st.price);
            }
        }
    }

    /// Roll the random draws for the coming week: returns, events, earnings,
    /// dividends. Returns the news/tip material via `PendingWeek`.
    pub fn roll_outlook(&self, rng: &mut Rng, target_week: u32) -> PendingWeek {
        let n = self.defs.len();
        let regime = self.regime.transition(rng);
        let index_ret = regime.drift() + regime.vol() * rng.normal();

        // Sector factors.
        let sectors = [
            Sector::Tech, Sector::Finance, Sector::Energy, Sector::Health,
            Sector::Consumer, Sector::Industrials, Sector::Staples,
            Sector::Broad, Sector::Metals, Sector::Crypto,
        ];
        let mut sector_ret = [0.0f64; 10];
        for (k, s) in sectors.iter().enumerate() {
            let base = match s {
                Sector::Crypto => 0.004 + 0.08 * rng.normal(), // crypto beta drifts up on hype
                Sector::Metals => -0.25 * index_ret + 0.012 * rng.normal(),
                _ => 0.55 * index_ret + 0.016 * rng.normal(),
            };
            sector_ret[k] = base;
        }
        let sec_idx = |s: Sector| -> usize {
            sectors.iter().position(|x| *x == s).unwrap()
        };

        // Per-instrument returns.
        let mut rets = vec![0.0f64; n];
        let mut events = Vec::new();
        let mut dividends = Vec::new();
        let mut earnings = Vec::new();

        for i in 0..n {
            let d = &self.defs[i];
            let st = &self.states[i];
            // Volatility clustering: a big last move makes this week jumpy.
            let cluster = (1.0 + 0.8 * (st.prev_ret.abs() / d.weekly_vol.max(1e-6))).min(2.5);
            let vol = d.weekly_vol * cluster;
            let idio = vol * rng.normal();
            let sr = sector_ret[sec_idx(d.sector)];
            let mut r = match d.kind {
                Kind::Crypto => sr + d.momentum * st.prev_ret + vol * rng.normal(),
                Kind::CryptoEtf => 0.9 * sr + 0.25 * vol * rng.normal(),
                _ => d.beta * index_ret + d.sector_beta * 0.5 * sr + idio * 0.9,
            };
            r += st.pending_impulse;
            rets[i] = r;
        }

        // Quarterly earnings for stocks in weeks 6/13/19/26.
        if is_earnings_week(target_week) {
            for i in 0..n {
                if self.defs[i].kind != Kind::Stock {
                    continue;
                }
                let surprise = rng.normal() * 0.55; // EPS surprise, in "beat/miss" units
                let jump = surprise * 0.14; // ~ +/-7% typical, up to ~ +/-20%
                rets[i] += jump;
                earnings.push((i, surprise));
                let beat = surprise > 0.0;
                events.push(Event {
                    category: EventCategory::Earnings,
                    symbol_idx: Some(i),
                    impulse: jump,
                    facts: format!(
                        "{} reported Q{} EPS {} consensus by {:.1}%",
                        self.defs[i].symbol,
                        (target_week + 1) / 6 + 1,
                        if beat { "BEAT" } else { "MISSED" },
                        surprise.abs() * 100.0
                    ),
                });
            }
        }

        // Dividends (quarterly, on the same weeks) for yield payers.
        if is_earnings_week(target_week) {
            for i in 0..n {
                let d = &self.defs[i];
                if d.div_yield > 0.0 && d.kind != Kind::Crypto {
                    let per_share = self.states[i].price * d.div_yield / 4.0;
                    dividends.push((i, per_share));
                    rets[i] -= per_share / self.states[i].price; // ex-dividend drop
                    events.push(Event {
                        category: EventCategory::Dividend,
                        symbol_idx: Some(i),
                        impulse: -per_share / self.states[i].price,
                        facts: format!(
                            "{} went ex-dividend at ${:.2}/share (yield {:.1}%)",
                            d.symbol, per_share, d.div_yield * 100.0
                        ),
                    });
                }
            }
        }

        // Random events: 1-3 per week.
        let n_events = 1 + rng.usize_below(3);
        for _ in 0..n_events {
            let ev = self.roll_event(rng, &mut rets);
            events.push(ev);
        }

        PendingWeek {
            target_week,
            regime,
            index_ret,
            rets,
            events,
            dividends,
            earnings,
        }
    }

    fn roll_event(&self, rng: &mut Rng, rets: &mut Vec<f64>) -> Event {
        let roll = rng.usize_below(100);
        if roll < 30 {
            // Macro event hitting the index / everything a bit.
            let (facts, impulse) = match rng.usize_below(6) {
                0 => ("The central bank HELD rates steady, citing sticky inflation".to_string(), 0.012),
                1 => ("The central bank HIKED rates 25bp, spooking growth stocks".to_string(), -0.020),
                2 => ("Oil prices spiked 8% on a supply scare".to_string(), -0.008),
                3 => ("A trade deal was signed, lifting industrials".to_string(), 0.010),
                4 => ("Geopolitical tensions flared; investors fled to gold".to_string(), -0.012),
                _ => ("Inflation data came in cooler than expected".to_string(), 0.014),
            };
            for r in rets.iter_mut() {
                *r += impulse * 0.6;
            }
            Event { category: EventCategory::Macro, symbol_idx: None, impulse, facts }
        } else if roll < 55 {
            // Sector event.
            let sec = [
                Sector::Tech, Sector::Energy, Sector::Health, Sector::Consumer,
                Sector::Industrials, Sector::Staples,
            ][rng.usize_below(6)];
            let up = rng.chance(0.5);
            let mag = rng.range(0.015, 0.05) * if up { 1.0 } else { -1.0 };
            let facts = format!(
                "{} sector {} on {}",
                sec.label(),
                if up { "rallied" } else { "sold off" },
                match rng.usize_below(3) {
                    0 => "a wave of analyst upgrades".to_string(),
                    1 => "fresh regulatory headwinds".to_string(),
                    _ => "rotating fund flows".to_string(),
                }
            );
            for (i, d) in self.defs.iter().enumerate() {
                if d.sector == sec {
                    rets[i] += mag * (0.5 + rng.f64());
                }
            }
            Event { category: EventCategory::Sector, symbol_idx: None, impulse: mag, facts }
        } else if roll < 85 {
            // Company event on one stock.
            let stocks: Vec<usize> = (0..self.defs.len())
                .filter(|&i| self.defs[i].kind == Kind::Stock)
                .collect();
            let i = *rng.pick(&stocks);
            let up = rng.chance(0.5);
            let mag = rng.range(0.03, 0.11) * if up { 1.0 } else { -1.0 };
            let facts = format!(
                "{} {} ",
                self.defs[i].symbol,
                match rng.usize_below(6) {
                    0 => "unveiled a blockbuster product at its annual showcase".to_string(),
                    1 => "lost a major contract to a rival".to_string(),
                    2 => "announced a big share buyback".to_string(),
                    3 => "is under investigation over accounting practices".to_string(),
                    4 => "raised full-year guidance after a strong quarter".to_string(),
                    _ => "cut guidance, blaming weak demand".to_string(),
                }
            );
            rets[i] += mag;
            Event { category: EventCategory::Company, symbol_idx: Some(i), impulse: mag, facts }
        } else {
            // Crypto event on one coin.
            let coins: Vec<usize> = (0..self.defs.len())
                .filter(|&i| self.defs[i].kind == Kind::Crypto)
                .collect();
            let i = *rng.pick(&coins);
            let up = rng.chance(0.55);
            let mag = rng.range(0.06, 0.22) * if up { 1.0 } else { -1.0 };
            let facts = format!(
                "{} {} ",
                self.defs[i].symbol,
                match rng.usize_below(6) {
                    0 => "surged after a major exchange listed it".to_string(),
                    1 => "crashed after an exchange hack".to_string(),
                    2 => "pumped on a celebrity endorsement".to_string(),
                    3 => "dumped after a viral 'rug pull' rumor".to_string(),
                    4 => "rallied on a network upgrade".to_string(),
                    _ => "moved on record ETF inflows".to_string(),
                }
            );
            rets[i] += mag;
            Event { category: EventCategory::Crypto, symbol_idx: Some(i), impulse: mag, facts }
        }
    }

    /// Apply a rolled week: update prices, history, regime.
    pub fn apply_week(&mut self, pending: &PendingWeek) {
        self.regime = pending.regime;
        self.index *= pending.index_ret.exp();
        self.index_history.push(self.index);
        for (i, st) in self.states.iter_mut().enumerate() {
            st.prev_ret = pending.rets[i];
            st.price = (st.price * pending.rets[i].exp()).max(0.05);
            st.history.push(st.price);
            st.pending_impulse *= 0.5;
        }
    }
}
