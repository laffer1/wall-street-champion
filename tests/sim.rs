//! Integration tests for the simulation core (no GPU needed).

use wall_street_champion::sim::game::{Game, Status, TOTAL_WEEKS};
use wall_street_champion::sim::portfolio::{Order, Side, GOAL_CENTS, START_CASH_CENTS, WEEKLY_LIVING_EXPENSES_CENTS};

// main.rs declares modules privately; re-declare the public path used in tests
// by way of a lib target built from the same sources.

#[test]
fn market_is_deterministic_per_seed() {
    let g1 = Game::new(1234, None);
    let g2 = Game::new(1234, None);
    assert_eq!(g1.prices(), g2.prices());
    let mut a = Game::new(1234, None);
    let mut b = Game::new(1234, None);
    for _ in 0..10 {
        a.end_week().unwrap();
        b.end_week().unwrap();
    }
    assert_eq!(a.prices(), b.prices());
}

#[test]
fn different_seeds_diverge() {
    let mut a = Game::new(1, None);
    let mut b = Game::new(2, None);
    a.end_week().unwrap();
    b.end_week().unwrap();
    assert_ne!(a.prices(), b.prices());
}

#[test]
fn full_game_runs_26_weeks_and_terminates() {
    let mut g = Game::new(7, None);
    assert_eq!(g.portfolio.cash_cents, START_CASH_CENTS);
    let mut weeks = 0;
    while g.status == Status::Playing {
        g.end_week().unwrap();
        weeks += 1;
        assert!(weeks <= TOTAL_WEEKS + 1, "game must terminate");
    }
    assert!(matches!(g.status, Status::Won | Status::Failed | Status::Bankrupt));
    // 26 weeks of living expenses were charged (unless bankrupt earlier).
    assert!(g.portfolio.cash_cents <= START_CASH_CENTS - WEEKLY_LIVING_EXPENSES_CENTS * 26 + 10_000_00);
}

#[test]
fn prices_stay_positive_and_history_grows() {
    let mut g = Game::new(99, None);
    for _ in 0..26 {
        if g.status != Status::Playing {
            break;
        }
        g.end_week().unwrap();
        for p in g.prices() {
            assert!(p > 0.0, "price must stay positive, got {}", p);
        }
        for st in &g.market.states {
            assert!(st.history.len() >= 9);
        }
    }
}

#[test]
fn buy_and_sell_execute_at_close() {
    let mut g = Game::new(5, None);
    let i = g.market.symbol_idx("XGL").unwrap();
    let price = g.prices()[i];
    g.portfolio.queue(Order { symbol_idx: i, side: Side::Buy, shares: 100.0 });
    let r = g.end_week().unwrap();
    assert_eq!(r.fills.len(), 1);
    assert!((g.portfolio.shares_of(i) - 100.0).abs() < 1e-9);
    let spent = 100.0 * price * 100.0;
    // cash reduced by trade + weekly expenses
    assert!(g.portfolio.cash_cents <= START_CASH_CENTS - spent as i64 - WEEKLY_LIVING_EXPENSES_CENTS);
}

#[test]
fn margin_rule_blocks_3x_leverage() {
    let mut g = Game::new(5, None);
    let i = g.market.symbol_idx("NEB").unwrap();
    let price = g.prices()[i];
    // Try to buy 30x equity worth: must be rejected.
    let shares = (30.0 * 1000.0) / price; // $30,000 on $100k... use bigger
    let shares = shares * 10.0;
    g.portfolio.queue(Order { symbol_idx: i, side: Side::Buy, shares });
    assert!(g.end_week().is_err());
}

#[test]
fn dividends_are_credited_on_earnings_weeks() {
    let mut g = Game::new(11, None);
    // Buy a dividend payer and hold through week 6 (first earnings week).
    let i = g.market.symbol_idx("GRF").unwrap();
    let price = g.prices()[i];
    g.portfolio.queue(Order { symbol_idx: i, side: Side::Buy, shares: 1000.0 / price });
    let mut got_div = 0i64;
    while g.week < 7 && g.status == Status::Playing {
        let r = g.end_week().unwrap();
        got_div += r.dividends_cents;
    }
    assert!(got_div > 0, "expected at least one dividend credit by week 6, got {}", got_div);
}

#[test]
fn news_and_tips_generated_each_week() {
    let mut g = Game::new(21, None);
    assert!(!g.news.is_empty());
    assert!(!g.tips.is_empty());
    g.end_week().unwrap();
    assert!(!g.news.is_empty());
    // Sources present
    assert!(g.news.iter().any(|n| n.source == wall_street_champion::sim::news::Source::Newspaper));
    assert!(g.news.iter().any(|n| n.source == wall_street_champion::sim::news::Source::Tv));
}

#[test]
fn win_condition_reachable() {
    let mut g = Game::new(3, None);
    // Simulate a windfall big enough to survive 26 weeks of expenses, then
    // run the clock out.
    g.portfolio.cash_cents = GOAL_CENTS + 26 * WEEKLY_LIVING_EXPENSES_CENTS + 1;
    while g.status == Status::Playing {
        g.end_week().unwrap();
    }
    assert_eq!(g.status, Status::Won);
}

#[test]
fn bankruptcy_when_wiped_out() {
    let mut g = Game::new(3, None);
    g.portfolio.cash_cents = -1; // already negative equity
    while g.status == Status::Playing {
        g.end_week().unwrap();
    }
    assert_eq!(g.status, Status::Bankrupt);
}
