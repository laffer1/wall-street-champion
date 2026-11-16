//! Financial news generation. Two outlets with different editorial voices:
//! the newspaper (factual) and the TV channel (hype). Headlines hint at the
//! direction of the coming week with some noise.
//!
//! If a local LLM (Ollama) is available, bodies of the top stories are written
//! by the model; otherwise procedural templates are used. See src/llm.rs.

use crate::llm::Llm;
use crate::rng::Rng;
use super::market::{Event, EventCategory, Market, PendingWeek};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Wire,
    Newspaper,
    Tv,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Wire => "WIRE",
            Source::Newspaper => "THE DAILY BULL & BEAR",
            Source::Tv => "MONEY-TV 24",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NewsItem {
    pub week: u32,
    pub source: Source,
    pub symbol: Option<usize>,
    pub headline: String,
    pub body: String,
    /// -1 bearish .. +1 bullish hint about the coming week.
    pub sentiment: f32,
}

fn fmt_pct(x: f64) -> String {
    format!("{:+.1}%", x * 100.0)
}

/// Build the week's news from the rolled outlook. `llm` is optional.
pub fn generate_week_news(
    rng: &mut Rng,
    market: &Market,
    pending: &PendingWeek,
    llm: Option<&Llm>,
) -> Vec<NewsItem> {
    let week = pending.target_week;
    let mut out = Vec::new();

    // 1. Event-driven stories (wire first, then both outlets spin them).
    //    Cap the quarterly noise: only the biggest earnings/dividends get ink.
    let mut events: Vec<&Event> = pending.events.iter().collect();
    for cat in [EventCategory::Earnings, EventCategory::Dividend] {
        let mut of_cat: Vec<&Event> = events.iter().copied().filter(|e| e.category == cat).collect();
        of_cat.sort_by(|a, b| b.impulse.abs().partial_cmp(&a.impulse.abs()).unwrap());
        let keep: Vec<&Event> = of_cat.into_iter().take(4).collect();
        events.retain(|e| e.category != cat || keep.iter().any(|k| std::ptr::eq(*k, *e)));
    }
    for ev in &events {
        let sym_name = ev
            .symbol_idx
            .map(|i| format!("{} ({})", market.defs[i].symbol, market.defs[i].name))
            .unwrap_or_else(|| "the market".to_string());
        let sentiment = (ev.impulse * 8.0).clamp(-1.0, 1.0) as f32;

        let (np_head, np_body) = newspaper_angle(rng, ev, market);
        let (tv_head, tv_body) = tv_angle(rng, ev, market);

        out.push(NewsItem {
            week,
            source: Source::Wire,
            symbol: ev.symbol_idx,
            headline: format!("BREAKING: {}", ev.facts),
            body: ev.facts.clone(),
            sentiment,
        });
        out.push(NewsItem {
            week,
            source: Source::Newspaper,
            symbol: ev.symbol_idx,
            headline: np_head,
            body: np_body,
            sentiment,
        });
        out.push(NewsItem {
            week,
            source: Source::Tv,
            symbol: ev.symbol_idx,
            headline: tv_head,
            body: tv_body,
            sentiment: (sentiment * 1.4).clamp(-1.0, 1.0), // TV exaggerates
        });
        let _ = sym_name;
    }

    // 2. Earnings previews/reactions for earnings weeks (top 4 by surprise).
    let mut earn: Vec<(usize, f64)> = pending.earnings.clone();
    earn.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    for (i, surprise) in earn.into_iter().take(4) {
        let d = &market.defs[i];
        let sent = (surprise.signum() as f32).clamp(-1.0, 1.0);
        out.push(NewsItem {
            week,
            source: Source::Newspaper,
            symbol: Some(i),
            headline: format!(
                "{} earnings: analysts split on whether the quarter lands",
                d.symbol
            ),
            body: format!(
                "{} ({}) reports this week. Whisper numbers point {}.",
                d.symbol,
                d.name,
                if surprise >= 0.0 { "to a beat" } else { "to a miss" }
            ),
            sentiment: sent * 0.8,
        });
    }

    // 3. "Analyst hint" stories: sentiment matches the coming return with
    //    78% accuracy (the rest is noise — the market is not fully predictable).
    let mut movers: Vec<(usize, f64)> = (0..market.defs.len())
        .map(|i| (i, pending.rets[i]))
        .filter(|(_, r)| r.abs() > 0.015)
        .collect();
    movers.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    movers.truncate(4);
    for (i, r) in movers {
        let d = &market.defs[i];
        let honest = rng.chance(0.78);
        let dir_up = if honest { r > 0.0 } else { r < 0.0 };
        let sent = if honest {
            (r.abs() * 10.0).clamp(0.2, 1.0) as f32 * if r > 0.0 { 1.0 } else { -1.0 }
        } else {
            -(r.abs() * 10.0).clamp(0.2, 1.0) as f32 * if r > 0.0 { 1.0 } else { -1.0 }
        };
        let head = if dir_up {
            format!("Why {} could be next week's big winner", d.symbol)
        } else {
            format!("{} faces serious headwinds, analysts warn", d.symbol)
        };
        let body = if dir_up {
            format!(
                "Institutional desks are quietly building positions in {} ({}). Order flow, options skew and channel checks all point the same way.",
                d.symbol, d.name
            )
        } else {
            format!(
                "{} ({}) is flashing warning signs: insiders selling, margins compressing and options flow turning defensive.",
                d.symbol, d.name
            )
        };
        out.push(NewsItem { week, source: Source::Newspaper, symbol: Some(i), headline: head.clone(), body: body.clone(), sentiment: sent });
        out.push(NewsItem {
            week,
            source: Source::Tv,
            symbol: Some(i),
            headline: if dir_up {
                format!("{} TO THE MOON?! Traders are piling in", d.symbol)
            } else {
                format!("PANIC SELLING in {}?! Here's what you must know", d.symbol)
            },
            body: if dir_up {
                format!("Our floor traders say {} ({}) is the hottest name on the Street this week. Everyone's talking about it!", d.symbol, d.name)
            } else {
                format!("{} ({}) is melting down fast on the screen — our sources say smart money is getting out. Are you?", d.symbol, d.name)
            },
            sentiment: (sent * 1.3).clamp(-1.0, 1.0),
        });
    }

    // 4. Market wrap for the week.
    out.push(NewsItem {
        week,
        source: Source::Newspaper,
        symbol: None,
        headline: format!(
            "Week {} outlook: {} regime, index {} expected",
            week,
            match pending.regime {
                super::market::Regime::Bull => "bullish",
                super::market::Regime::Sideways => "choppy",
                super::market::Regime::Bear => "bearish",
            },
            fmt_pct(pending.index_ret)
        ),
        body: format!(
            "The Globex 500 (XGL) is projected around {} this week. Volatility is expected to {}.",
            fmt_pct(pending.index_ret),
            if pending.regime == super::market::Regime::Bear { "stay elevated" } else { "normalize" }
        ),
        sentiment: (pending.index_ret * 12.0).clamp(-1.0, 1.0) as f32,
    });

    // 5. Optional: let a local model write the bodies of the top 3 stories.
    if let Some(llm) = llm {
        let mut rewritten = 0;
        for item in out.iter_mut().take(12) {
            if rewritten >= 3 {
                break;
            }
            if item.source == Source::Wire {
                continue;
            }
            if let Some(text) = llm.news_body(&item.headline, &item.body) {
                item.body = text;
                rewritten += 1;
            }
        }
    }

    out
}

fn newspaper_angle(rng: &mut Rng, ev: &Event, market: &Market) -> (String, String) {
    let sym = ev.symbol_idx.map(|i| market.defs[i].symbol).unwrap_or("MARKETS");
    let heads = match ev.category {
        EventCategory::Macro => vec![
            format!("Markets steady as investors digest: {}", ev.facts),
            format!("Macro shock: {}", ev.facts),
        ],
        EventCategory::Sector => vec![format!("Sector rotation underway: {}", ev.facts)],
        EventCategory::Company => vec![
            format!("{} in focus: {}", sym, ev.facts),
            format!("{} moves on company news", sym),
        ],
        EventCategory::Earnings => vec![format!("{} quarterly results in focus", sym)],
        EventCategory::Dividend => vec![format!("{} income play: {}", sym, ev.facts)],
        EventCategory::Crypto => vec![format!("Crypto corner: {}", ev.facts)],
    };
    let head = rng.pick(&heads).clone();
    let body = format!(
        "{} — {} The read-through for portfolios is {}.",
        ev.facts,
        match ev.category {
            EventCategory::Macro => "broad positioning matters more than single names this week".to_string(),
            EventCategory::Sector => "winners and losers inside the sector will diverge sharply".to_string(),
            EventCategory::Company => "the idiosyncratic move is likely to dominate the ticker".to_string(),
            EventCategory::Earnings => "the print will set the tone for the next quarter".to_string(),
            EventCategory::Dividend => "total return, not just price, is what counts".to_string(),
            EventCategory::Crypto => "crypto volatility is running hot; size positions accordingly".to_string(),
        },
        if ev.impulse >= 0.0 { "constructive" } else { "negative" }
    );
    (head, body)
}

fn tv_angle(rng: &mut Rng, ev: &Event, market: &Market) -> (String, String) {
    let sym = ev.symbol_idx.map(|i| market.defs[i].symbol).unwrap_or("THE MARKET");
    let up = ev.impulse >= 0.0;
    let heads = if up {
        vec![
            format!("BULLISH! {} — this could be huge", ev.facts),
            format!("{} EXPLODES on fresh news!", sym),
        ]
    } else {
        vec![
            format!("BEARISH ALERT: {} — brace for impact", ev.facts),
            format!("{} CRASHES on the tape!", sym),
        ]
    };
    let head = rng.pick(&heads).clone();
    let body = format!(
        "Anchor: \"{} Our traders are watching {} like a hawk. If you own it, you need to know this NOW.\"",
        ev.facts, sym
    );
    (head, body)
}
