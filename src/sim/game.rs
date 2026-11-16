//! Game orchestration: ties market + portfolio + news + tipsters together,
//! enforces the rules (one trade batch per week, weekly expenses, 26 weeks,
//! $1M goal).

use crate::llm::Llm;
use crate::rng::Rng;
use super::market::{Market, PendingWeek};
use super::news::{generate_week_news, NewsItem, Source};
use super::people::{generate_people, generate_tips, reveal_tips, Person, Tip};
use super::portfolio::{Fill, Portfolio, GOAL_CENTS, WEEKLY_LIVING_EXPENSES_CENTS};

pub const TOTAL_WEEKS: u32 = 26; // six months

#[derive(Clone, Debug)]
pub struct WeekReport {
    pub week: u32,
    pub fills: Vec<Fill>,
    pub dividends_cents: i64,
    pub expenses_cents: i64,
    pub interest_cents: i64,
    pub net_worth_cents: i64,
    pub returns: Vec<f64>,
    pub news: Vec<NewsItem>,
    pub tips: Vec<Tip>,
    pub events: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Playing,
    Won,
    Bankrupt,
    Failed, // 26 weeks over, under $1M
}

pub struct Game {
    pub market: Market,
    pub portfolio: Portfolio,
    pub people: Vec<Person>,
    pub news: Vec<NewsItem>,
    pub tips: Vec<Tip>,
    pub week: u32,
    pub pending: PendingWeek,
    pub rng: Rng,
    pub llm: Option<Llm>,
    pub history_net_worth: Vec<i64>,
    pub status: Status,
    pub followed_tip: Option<usize>,
}

impl Game {
    pub fn new(seed: u64, llm: Option<Llm>) -> Self {
        let mut rng = Rng::new(seed);
        let mut market = Market::new();
        market.backfill(&mut rng);
        let mut people = generate_people(&mut rng);
        let pending = market.roll_outlook(&mut rng, 1);
        let news = generate_week_news(&mut rng, &market, &pending, llm.as_ref());
        let tips = generate_tips(&mut rng, &mut people, &market, &pending);
        let mut g = Game {
            market,
            portfolio: Portfolio::new(),
            people,
            news,
            tips,
            week: 1,
            pending,
            rng,
            llm,
            history_net_worth: Vec::new(),
            status: Status::Playing,
            followed_tip: None,
        };
        g.history_net_worth.push(g.net_worth_cents());
        g
    }

    pub fn prices(&self) -> Vec<f64> {
        self.market.states.iter().map(|s| s.price).collect()
    }

    pub fn net_worth_cents(&self) -> i64 {
        self.portfolio.net_worth_cents(&self.prices())
    }

    /// End the week: execute queued trades at this week's close, charge
    /// expenses + margin interest, credit dividends, apply the rolled market
    /// week, then roll the next week's outlook and generate its news/tips.
    pub fn end_week(&mut self) -> Result<WeekReport, String> {
        let prices = self.prices();
        let fills = self.portfolio.execute(&prices)?;

        let (expenses, interest) = self.portfolio.weekly_charges(&prices);
        let dividends = self.portfolio.credit_dividends(&self.pending.dividends);

        // Apply the market week that was telegraphed by this week's news.
        let returns = self.pending.rets.clone();
        let events: Vec<String> = self.pending.events.iter().map(|e| e.facts.clone()).collect();
        self.market.apply_week(&self.pending);

        // Reveal last week's tips.
        reveal_tips(&mut self.people, &mut self.tips, &self.pending);

        let nw = self.net_worth_cents();
        self.history_net_worth.push(nw);

        // Status checks.
        self.week += 1;
        if nw <= 0 {
            self.status = Status::Bankrupt;
        } else if self.week > TOTAL_WEEKS {
            self.status = if nw >= GOAL_CENTS { Status::Won } else { Status::Failed };
        }

        // Roll the next week only if the game continues.
        let next_week = self.week;
        if self.status == Status::Playing {
            self.pending = self.market.roll_outlook(&mut self.rng, next_week);
            self.news = generate_week_news(&mut self.rng, &self.market, &self.pending, self.llm.as_ref());
            self.tips = generate_tips(&mut self.rng, &mut self.people, &self.market, &self.pending);
            self.followed_tip = None;
        }

        Ok(WeekReport {
            week: next_week,
            fills,
            dividends_cents: dividends,
            expenses_cents: expenses,
            interest_cents: interest,
            net_worth_cents: nw,
            returns,
            news: self.news.clone(),
            tips: self.tips.clone(),
            events,
        })
    }

    /// Follow tip number `i`: invest 10% of equity in that direction.
    pub fn follow_tip(&mut self, i: usize) -> Result<String, String> {
        if i >= self.tips.len() {
            return Err("no such tip".to_string());
        }
        let tip = self.tips[i].clone();
        let prices = self.prices();
        let equity = self.portfolio.equity_cents(&prices).max(0) as f64 / 100.0;
        let amount = (equity * 0.10).max(0.0);
        let price = prices[tip.symbol_idx];
        if amount < 1.0 {
            return Err("not enough equity to follow a tip".to_string());
        }
        let shares = amount / price;
        use super::portfolio::{Order, Side};
        if tip.up {
            self.portfolio.queue(Order { symbol_idx: tip.symbol_idx, side: Side::Buy, shares });
            self.followed_tip = Some(i);
            Ok(format!(
                "Following {}'s tip: queued BUY {} shares of {} (~${:.2})",
                self.people[tip.person].name, shares, self.market.defs[tip.symbol_idx].symbol, amount
            ))
        } else {
            let have = self.portfolio.shares_of(tip.symbol_idx);
            if have <= 0.0 {
                return Err(format!(
                    "tip says dump {}, but you hold none — nothing to sell",
                    self.market.defs[tip.symbol_idx].symbol
                ));
            }
            self.portfolio.queue(Order { symbol_idx: tip.symbol_idx, side: Side::Sell, shares: have.min(shares) });
            self.followed_tip = Some(i);
            Ok(format!(
                "Following {}'s tip: queued SELL of {} ({} shares)",
                self.people[tip.person].name, self.market.defs[tip.symbol_idx].symbol, have.min(shares)
            ))
        }
    }

    pub fn toggle_trust(&mut self, person: usize) {
        if person < self.people.len() {
            self.people[person].trusted = !self.people[person].trusted;
        }
    }

    pub fn news_by(&self, source: Source) -> Vec<&NewsItem> {
        self.news.iter().filter(|n| n.source == source).collect()
    }
}

pub fn weekly_expenses_note() -> String {
    format!("living expenses ${:.0}/week, no other income", WEEKLY_LIVING_EXPENSES_CENTS as f64 / 100.0)
}
