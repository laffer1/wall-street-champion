//! Terminal UI — the game is fully playable without a GPU. Also provides
//! `--bot` for automated 26-week runs (verification/demo).

use std::io::{self, BufRead, Write};
use crate::llm::Llm;
use crate::sim::game::{Game, Status, TOTAL_WEEKS};
use crate::sim::market::Regime;
use crate::sim::news::Source;
use crate::sim::portfolio::{fmt_money, fmt_price, Order, Side};

const RST: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[91m";
const GRN: &str = "\x1b[92m";
const YEL: &str = "\x1b[93m";
const BLU: &str = "\x1b[94m";
const MAG: &str = "\x1b[95m";
const CYN: &str = "\x1b[96m";

pub fn run(seed: u64, llm: Option<Llm>, bot: bool) -> i32 {
    let mut game = Game::new(seed, llm);
    if bot {
        return bot_play(&mut game);
    }
    println!("{CYN}╔══════════════════════════════════════════════════════════════════════════╗{RST}");
    println!("{CYN}║{RST}  🐂 {BOLD}{YEL}WALL STREET CHAMPION{RST} 🐻   {CYN}—  $100,000 TO $1,000,000 IN {TOTAL_WEEKS} WEEKS{RST}      {CYN}║{RST}");
    println!("{CYN}║{RST}  {DIM}Living expenses: $1,500/wk • Margin: 2x (0.5%/wk) • Type 'help'{RST}         {CYN}║{RST}");
    println!("{CYN}╚══════════════════════════════════════════════════════════════════════════╝{RST}");
    loop {
        print_screen(&game);
        print!("{BOLD}{YEL}wsc>{RST} ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        match handle_command(&mut game, &line) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => println!("  ! {}", e),
        }
        match game.status {
            Status::Won => {
                print_end(&game, true, "A trading firm just wired you a signing bonus. HIRED!");
                return 0;
            }
            Status::Bankrupt => {
                print_end(&game, false, "Bankrupt. The landlord changed the locks. PENNILESS.");
                return 1;
            }
            Status::Failed => {
                print_end(&game, false, "26 weeks are over and you are short of $1,000,000. PENNILESS.");
                return 1;
            }
            Status::Playing => {}
        }
    }
    0
}

fn print_end(game: &Game, won: bool, msg: &str) {
    println!("\n**********************************************************************");
    println!("  GAME OVER — {}", if won { "YOU WIN" } else { "YOU LOSE" });
    println!("  {}", msg);
    println!("  Final net worth: {} (goal was $1,000,000)", fmt_money(game.net_worth_cents()));
    println!("  Tipster scorecards:");
    for p in &game.people {
        if p.tips_given > 0 {
            println!(
                "    {:<18} ({}): {}/{} tips right ({:.0}%){}",
                p.name, p.archetype, p.tips_correct, p.tips_given,
                100.0 * p.tips_correct as f64 / p.tips_given as f64,
                if p.trusted { " [you trusted them]" } else { "" }
            );
        }
    }
    println!("**********************************************************************");
}

fn print_screen(g: &Game) {
    let prices = g.prices();
    let nw = g.net_worth_cents();
    let pct = 100.0 * nw as f64 / 1_000_000_00.0;
    let reg = match g.market.regime {
        Regime::Bull => format!("{GRN}▲ BULL{RST}"),
        Regime::Bear => format!("{RED}▼ BEAR{RST}"),
        Regime::Sideways => format!("{YEL}◆ SIDEWAYS{RST}"),
    };
    let nw_col = if nw >= 100_000_00 { GRN } else { RED };
    let bar_n = ((pct / 5.0).round() as usize).clamp(0, 20);
    let bar = format!("{GRN}{}{DIM}{}{RST}", "█".repeat(bar_n), "░".repeat(20 - bar_n));
    println!(
        "\n{BLU}┏━━ {BOLD}{YEL}WEEK {:>2}/{}{RST} {BLU}━━━{RST} Regime: {} {BLU}━━━{RST} Goal [{}] {BOLD}{:.1}%{RST}",
        g.week.min(TOTAL_WEEKS),
        TOTAL_WEEKS,
        reg,
        bar,
        pct
    );
    println!(
        "{BLU}┃{RST} Cash {CYN}{:<14}{RST} Debt {YEL}{:<12}{RST} Net Worth {BOLD}{nw_col}{:<14}{RST}",
        fmt_money(g.portfolio.cash_cents),
        fmt_money(g.portfolio.debt_cents()),
        fmt_money(nw),
    );
    if !g.portfolio.queued.is_empty() {
        println!("{BLU}┃{RST} {YEL}Queued orders:{RST}");
        for o in &g.portfolio.queued {
            let scol = match o.side { Side::Buy => GRN, Side::Sell => RED };
            println!(
                "{BLU}┃{RST}   {scol}{:?}{RST} {BOLD}{}{RST} {:.4} shares @ {}",
                o.side, g.market.defs[o.symbol_idx].symbol, o.shares, fmt_price(prices[o.symbol_idx])
            );
        }
    }
    println!("{BLU}┗━━{RST} Holdings: {}", holdings_line(g));
    print_tips(g);
}

fn holdings_line(g: &Game) -> String {
    let prices = g.prices();
    if g.portfolio.holdings.is_empty() {
        return format!("{DIM}(none — 100% cash){RST}");
    }
    let mut v: Vec<_> = g.portfolio.holdings.iter().collect();
    v.sort_by_key(|&(i, _)| *i);
    v.iter()
        .map(|(&i, &sh)| format!("{BOLD}{CYN}{}{RST} x{:.2} ({GRN}{}{RST})", g.market.defs[i].symbol, sh, fmt_money((sh * prices[i] * 100.0) as i64)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_tips(g: &Game) {
    if g.tips.is_empty() {
        return;
    }
    println!("{MAG}💬 Street Contacts & Tips{RST} {DIM}(follow N / trust P):{RST}");
    for (i, t) in g.tips.iter().enumerate() {
        let p = &g.people[t.person];
        let outcome = match t.was_correct {
            Some(true) => format!(" {GRN}[RIGHT]{RST}"),
            Some(false) => format!(" {RED}[WRONG]{RST}"),
            None => String::new(),
        };
        let dir = if t.up { format!("{GRN}▲ UP{RST}") } else { format!("{RED}▼ DOWN{RST}") };
        let tr = if p.trusted { format!(", {YEL}★TRUSTED{RST}") } else { String::new() };
        println!(
            "  {YEL}[{}]{RST} {BOLD}{}{RST} ({DIM}{}{RST}{}): \"{}\"{}  -> {BOLD}{}{RST} {}",
            i + 1, p.name, p.archetype, tr,
            t.text, outcome, g.market.defs[t.symbol_idx].symbol, dir
        );
    }
}

fn print_market(g: &Game) {
    let prices = g.prices();
    println!("\n{CYN}══ MARKET WATCHLIST (week {}) — regime: {:?} ══{RST}", g.week, g.market.regime);
    println!("{BOLD}{:<6} {:<28} {:<12} {:>10} {:>9}  hist{RST}", "SYM", "NAME", "KIND", "PRICE", "WK CHG");
    for (i, d) in g.market.defs.iter().enumerate() {
        let st = &g.market.states[i];
        let wk = if st.history.len() >= 2 {
            st.history[st.history.len() - 1] / st.history[st.history.len() - 2] - 1.0
        } else {
            0.0
        };
        let sel = if g.portfolio.shares_of(i) > 0.0 { format!("{YEL}★{RST}") } else { String::new() };
        let ccol = if wk >= 0.0 { GRN } else { RED };
        println!(
            "{BOLD}{:<6}{RST} {:<28} {DIM}{:<12}{RST} {:>10} {ccol}{:>+8.2}%  {}{RST} {}",
            d.symbol, truncate(&d.name, 28), d.kind.label(), fmt_price(prices[i]), wk * 100.0,
            spark(&st.history), sel
        );
    }
    println!("{DIM}(★ = you hold it. XGL index level: {:.2}){RST}", g.market.index);
}

fn print_news(g: &Game, source: Source) {
    println!("\n{YEL}═════ 📰 {} — week {} ═════{RST}", source.label(), g.week);
    let items = g.news_by(source);
    if items.is_empty() {
        println!("{DIM}(no stories){RST}");
    }
    for n in items.iter().take(14) {
        let sym = n.symbol.map(|i| g.market.defs[i].symbol).unwrap_or("MKT");
        let (arrow, col) = if n.sentiment > 0.25 {
            ("▲", GRN)
        } else if n.sentiment < -0.25 {
            ("▼", RED)
        } else {
            ("•", CYN)
        };
        println!("{col}[{:>4}] {}{RST} {BOLD}{}{RST}", sym, arrow, n.headline);
        println!("        {DIM}{}{RST}", truncate(&n.body, 110));
    }
}

fn print_help() {
    println!(
        "commands:
  m            market table
  p            portfolio detail
  n            newspaper (The Daily Bull & Bear)
  v            TV news (MONEY-TV 24)
  w            wire flashes
  t            tips
  buy SYM $    queue buy $AMOUNT of SYM (fills at week close)
  sell SYM $   queue sell $AMOUNT of SYM (or 'all')
  q            show queued orders
  c            clear queued orders
  follow N     follow tip N (invests 10% of equity that way)
  trust P      toggle trust on tipster P (1-8)
  e            END WEEK: execute trades, charge expenses, advance market
  help         this
  quit         exit"
    );
}

fn handle_command(g: &mut Game, line: &str) -> Result<bool, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts[0].to_lowercase().as_str() {
        "help" | "h" | "?" => print_help(),
        "quit" | "q!" | "exit" => return Ok(false),
        "m" | "market" => print_market(g),
        "p" | "portfolio" => {
            print_screen(g);
        }
        "n" | "news" => print_news(g, Source::Newspaper),
        "v" | "tv" => print_news(g, Source::Tv),
        "w" | "wire" => print_news(g, Source::Wire),
        "t" | "tips" => print_tips(g),
        "q" => {
            if g.portfolio.queued.is_empty() {
                println!("(no queued orders)");
            }
            for o in &g.portfolio.queued {
                println!("  {:?} {} {:.4}", o.side, g.market.defs[o.symbol_idx].symbol, o.shares);
            }
        }
        "c" | "clear" => {
            g.portfolio.queued.clear();
            println!("queue cleared");
        }
        "buy" => {
            if parts.len() < 3 {
                return Err("usage: buy SYM $AMOUNT".into());
            }
            let i = g.market.symbol_idx(parts[1].to_uppercase().as_str())
                .ok_or_else(|| format!("unknown symbol {}", parts[1]))?;
            let amt: f64 = parts[2].trim_start_matches('$').parse().map_err(|_| "bad amount")?;
            let price = g.prices()[i];
            let shares = amt / price;
            g.portfolio.queue(Order { symbol_idx: i, side: Side::Buy, shares });
            println!("queued BUY {} {:.4} shares (~${:.2})", g.market.defs[i].symbol, shares, amt);
        }
        "sell" => {
            if parts.len() < 3 {
                return Err("usage: sell SYM $AMOUNT|all".into());
            }
            let i = g.market.symbol_idx(parts[1].to_uppercase().as_str())
                .ok_or_else(|| format!("unknown symbol {}", parts[1]))?;
            let have = g.portfolio.shares_of(i);
            if have <= 0.0 {
                return Err(format!("you hold no {}", g.market.defs[i].symbol));
            }
            let shares = if parts[2] == "all" {
                have
            } else {
                let amt: f64 = parts[2].trim_start_matches('$').parse().map_err(|_| "bad amount")?;
                (amt / g.prices()[i]).min(have)
            };
            g.portfolio.queue(Order { symbol_idx: i, side: Side::Sell, shares });
            println!("queued SELL {} {:.4} shares", g.market.defs[i].symbol, shares);
        }
        "follow" => {
            let n: usize = parts.get(1).and_then(|s| s.parse().ok()).ok_or("usage: follow N")?;
            let msg = g.follow_tip(n - 1)?;
            println!("{}", msg);
        }
        "trust" => {
            let n: usize = parts.get(1).and_then(|s| s.parse().ok()).ok_or("usage: trust P (1-8)")?;
            g.toggle_trust(n - 1);
            println!("trust toggled for {}", g.people.get(n - 1).map(|p| p.name.as_str()).unwrap_or("?"));
        }
        "e" | "end" => {
            let r = g.end_week()?;
            println!(
                "\n>> Week {} began. Fills: {}, dividends {}, expenses -{}, interest -{}, net worth {}",
                r.week, r.fills.len(), fmt_money(r.dividends_cents),
                fmt_money(r.expenses_cents), fmt_money(r.interest_cents), fmt_money(r.net_worth_cents)
            );
            for ev in &r.events {
                println!("   * {}", ev);
            }
            print_tips(g);
        }
        other => return Err(format!("unknown command '{}' (try 'help')", other)),
    }
    Ok(true)
}

/// Simple automated player: each week it ranks instruments by aggregated news
/// sentiment and buys the most bullish one with half its buying power, selling
/// its worst holding first. Proves the full 26-week loop end to end.
fn bot_play(g: &mut Game) -> i32 {
    println!("BOT MODE: auto-playing {} weeks (seeded market)...", TOTAL_WEEKS);
    while g.status == Status::Playing {
        // Aggregate news sentiment per instrument.
        let mut score = vec![0.0f32; g.market.defs.len()];
        for n in &g.news {
            if let Some(i) = n.symbol {
                score[i] += n.sentiment;
            }
        }
        let prices = g.prices();
        let bp = g.portfolio.buying_power_cents(&prices).max(0) as f64 / 100.0;
        let equity = g.net_worth_cents().max(1) as f64 / 100.0;

        // Sell the worst holding if its news turned bearish.
        let worst = g
            .portfolio
            .holdings
            .keys()
            .min_by(|a, b| score[**a].partial_cmp(&score[**b]).unwrap())
            .copied();
        if let Some(i) = worst {
            if score[i] < -0.5 {
                let sh = g.portfolio.shares_of(i);
                g.portfolio.queue(Order { symbol_idx: i, side: Side::Sell, shares: sh });
            }
        }
        // Buy the most bullish name with half of buying power.
        if bp > 100.0 {
            if let Some(best) = score.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i) {
                if score[best] > 0.3 {
                    let amt = (bp * 0.5).min(equity * 0.9);
                    let shares = amt / prices[best];
                    g.portfolio.queue(Order { symbol_idx: best, side: Side::Buy, shares });
                }
            }
        }
        let r = match g.end_week() {
            Ok(r) => r,
            Err(e) => {
                println!("week {}: order rejected: {}", g.week, e);
                g.portfolio.queued.clear();
                g.end_week().expect("advance without orders");
                continue;
            }
        };
        println!(
            "week {:>2}: net worth {:>14}  fills {}  div {}  news {}  tips {}",
            r.week, fmt_money(r.net_worth_cents), r.fills.len(),
            fmt_money(r.dividends_cents), r.news.len(), r.tips.len()
        );
    }
    print_end(g, g.status == Status::Won, match g.status {
        Status::Won => "The bot got hired by a trading firm.",
        Status::Bankrupt => "The bot went bankrupt.",
        _ => "The bot failed to reach $1M.",
    });
    if g.status == Status::Won { 0 } else { 1 }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n - 1).collect::<String>())
    }
}

fn spark(history: &[f64]) -> String {
    const GLYPHS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let tail: Vec<f64> = history.iter().rev().take(16).copied().collect();
    let tail: Vec<f64> = tail.into_iter().rev().collect();
    if tail.is_empty() {
        return String::new();
    }
    let min = tail.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = tail.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = (max - min).max(1e-9);
    tail.iter()
        .map(|&p| GLYPHS[((p - min) / range * 7.0).round() as usize])
        .collect()
}
