//! CPU-side immediate-mode quad batch + the whole game UI layout.
//! Everything (panels, charts, text, ticker) is drawn as quads; text uses the
//! vendored 8x8 font atlas (mode=1 vertices sample it in the scene shader).

use crate::sim::game::{Game, Status, TOTAL_WEEKS};
use crate::sim::news::Source;
use crate::sim::portfolio::{fmt_money, fmt_price};

pub const FONT_W: f32 = 8.0;
pub const FONT_H: f32 = 8.0;

// palette (linear-ish values, tuned for B8G8R8A8_UNORM output)
pub const BG: [f32; 4] = [0.045, 0.055, 0.075, 1.0];
pub const PANEL: [f32; 4] = [0.085, 0.10, 0.13, 1.0];
pub const PANEL2: [f32; 4] = [0.11, 0.13, 0.17, 1.0];
pub const EDGE: [f32; 4] = [0.19, 0.23, 0.29, 1.0];
pub const TXT: [f32; 4] = [0.85, 0.88, 0.93, 1.0];
pub const DIM: [f32; 4] = [0.45, 0.50, 0.58, 1.0];
pub const ACCENT: [f32; 4] = [0.22, 0.55, 1.0, 1.0];
pub const GREEN: [f32; 4] = [0.20, 0.75, 0.35, 1.0];
pub const RED: [f32; 4] = [0.95, 0.30, 0.25, 1.0];
pub const YELLOW: [f32; 4] = [0.85, 0.65, 0.15, 1.0];
pub const TICKER_BG: [f32; 4] = [0.02, 0.03, 0.05, 1.0];

pub struct UiState {
    pub selected: usize,
    pub news_source: Source,
    pub tip_sel: usize,
    pub msg: String,
    pub show_help: bool,
    pub frame: u64,
    pub fsr: bool,
    pub fsr_scale: f32,
    pub llm_on: bool,
}

impl UiState {
    pub fn new() -> Self {
        UiState {
            selected: 0,
            news_source: Source::Newspaper,
            tip_sel: 0,
            msg: "Welcome. Queue trades, then press E to end the week.".to_string(),
            show_help: false,
            frame: 0,
            fsr: true,
            fsr_scale: 0.66,
            llm_on: false,
        }
    }
}

/// 10 f32 per vertex: pos(2), color(4), uv(2), mode(1), pad(1). 6 verts/quad.
pub struct Batch {
    pub v: Vec<f32>,
}

impl Batch {
    pub fn new() -> Self {
        Batch { v: Vec::with_capacity(1 << 20) }
    }

    fn quad(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 4], uv0: (f32, f32), uv1: (f32, f32), mode: f32) {
        let verts = [
            (x0, y0, uv0.0, uv0.1),
            (x1, y0, uv1.0, uv0.1),
            (x0, y1, uv0.0, uv1.1),
            (x0, y1, uv0.0, uv1.1),
            (x1, y0, uv1.0, uv0.1),
            (x1, y1, uv1.0, uv1.1),
        ];
        for (x, y, u, w) in verts {
            self.v.extend_from_slice(&[x, y, col[0], col[1], col[2], col[3], u, w, mode, 0.0]);
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.quad(x, y, x + w, y + h, col, (0.0, 0.0), (0.0, 0.0), 0.0);
    }

    pub fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.rect(x, y, w, 1.0, col);
        self.rect(x, y + h - 1.0, w, 1.0, col);
        self.rect(x, y, 1.0, h, col);
        self.rect(x + w - 1.0, y, 1.0, h, col);
    }

    /// Draw text. Returns width in pixels. Handles '\n'.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, s: &str, col: [f32; 4]) -> f32 {
        let mut cx = x;
        let mut cy = y;
        for ch in s.chars() {
            if ch == '\n' {
                cx = x;
                cy += FONT_H * scale;
                continue;
            }
            let code = (ch as u32).min(127) as usize;
            let u0 = ((code % 16) * 8) as f32;
            let v0 = ((code / 16) * 8) as f32;
            let w = FONT_W * scale;
            let h = FONT_H * scale;
            self.quad(cx, cy, cx + w, cy + h, col, (u0, v0), (u0 + 8.0, v0 + 8.0), 1.0);
            cx += w;
        }
        cx - x
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, col: [f32; 4]) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let nx = -dy / len * thick * 0.5;
        let ny = dx / len * thick * 0.5;
        let verts = [
            (x0 + nx, y0 + ny),
            (x1 + nx, y1 + ny),
            (x0 - nx, y0 - ny),
            (x0 - nx, y0 - ny),
            (x1 + nx, y1 + ny),
            (x1 - nx, y1 - ny),
        ];
        for (x, y) in verts {
            self.v.extend_from_slice(&[x, y, col[0], col[1], col[2], col[3], 0.0, 0.0, 0.0, 0.0]);
        }
    }

    /// Line chart of `data` inside rect (x,y,w,h).
    pub fn chart(&mut self, x: f32, y: f32, w: f32, h: f32, data: &[f64], col: [f32; 4]) {
        if data.len() < 2 {
            return;
        }
        let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = (max - min).max(1e-9);
        let n = data.len();
        let px = |i: usize| x + w * i as f32 / (n - 1) as f32;
        let py = |v: f64| y + h - (v - min) as f32 / range as f32 * h;
        for i in 0..n - 1 {
            self.line(px(i), py(data[i]), px(i + 1), py(data[i + 1]), 1.5, col);
        }
    }
}

fn trunc(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        s.to_string()
    } else {
        format!("{}..", s.chars().take(max_chars - 2).collect::<String>())
    }
}

fn pct(x: f64) -> String {
    format!("{:+.2}%", x * 100.0)
}

/// Build the entire frame's geometry for a logical resolution (w, h).
pub fn build(game: &Game, ui: &UiState, w: f32, h: f32) -> Batch {
    let mut b = Batch::new();
    let prices = game.prices();
    let ticker_h = 26.0;
    let top_h = 42.0;
    let bottom_h = 64.0;

    b.rect(0.0, 0.0, w, h, BG);

    // ---------- top bar ----------
    b.rect(0.0, 0.0, w, top_h, PANEL);
    b.rect(0.0, top_h - 1.0, w, 1.0, EDGE);
    b.text(10.0, 8.0, 1.6, "WALL STREET CHAMPION", ACCENT);
    let nw = game.net_worth_cents();
    let goal_frac = (nw as f64 / 1_000_000_00.0).min(1.0) as f32;
    b.text(10.0, 28.0, 1.0, &format!("WEEK {}/{}   cash {}  debt {}  net worth {}", game.week.min(TOTAL_WEEKS), TOTAL_WEEKS, fmt_money(game.portfolio.cash_cents), fmt_money(game.portfolio.debt_cents()), fmt_money(nw)), TXT);
    // goal progress bar
    let bar_x = w - 300.0;
    let bar_w = 290.0;
    b.rect(bar_x, 12.0, bar_w, 12.0, PANEL2);
    b.rect(bar_x, 12.0, bar_w * goal_frac, 12.0, if nw >= 1_000_000_00 { GREEN } else { ACCENT });
    b.outline(bar_x, 12.0, bar_w, 12.0, EDGE);
    b.text(bar_x, 27.0, 1.0, &format!("GOAL $1,000,000 ({:.1}%)   FSR1.x {} {:.2}x   LLM {}", goal_frac * 100.0, if ui.fsr { "ON" } else { "OFF" }, ui.fsr_scale, if ui.llm_on { "ON" } else { "off" }), DIM);

    let body_top = top_h + 8.0;
    let body_bot = h - ticker_h - bottom_h - 8.0;
    let left_w = 320.0;
    let right_x = w - 440.0;
    let mid_x = left_w + 16.0;
    let mid_w = right_x - mid_x - 16.0;

    // ---------- left panel: portfolio ----------
    b.rect(8.0, body_top, left_w, body_bot - body_top, PANEL);
    b.outline(8.0, body_top, left_w, body_bot - body_top, EDGE);
    let mut y = body_top + 8.0;
    b.text(16.0, y, 1.25, "PORTFOLIO", ACCENT);
    y += 20.0;
    b.text(16.0, y, 1.0, &format!("cash   {}", fmt_money(game.portfolio.cash_cents)), TXT);
    y += 11.0;
    b.text(16.0, y, 1.0, &format!("debt   {}", fmt_money(game.portfolio.debt_cents())), if game.portfolio.debt_cents() > 0 { YELLOW } else { TXT });
    y += 11.0;
    b.text(16.0, y, 1.0, &format!("worth  {}", fmt_money(nw)), if nw >= 100_000_00 { GREEN } else { RED });
    y += 16.0;
    b.text(16.0, y, 1.0, "HOLDINGS", DIM);
    y += 12.0;
    if game.portfolio.holdings.is_empty() {
        b.text(16.0, y, 1.0, "(none — all cash)", DIM);
        y += 12.0;
    } else {
        let mut rows: Vec<(usize, f64)> = game.portfolio.holdings.iter().map(|(&i, &s)| (i, s)).collect();
        rows.sort_by_key(|&(i, _)| i);
        for (i, sh) in rows {
            let d = &game.market.defs[i];
            let val = (sh * prices[i] * 100.0) as i64;
            let hl = if i == ui.selected { ACCENT } else { TXT };
            b.text(16.0, y, 1.0, &format!("{:<5} {:>8.2}  {}", d.symbol, sh, fmt_money(val)), hl);
            y += 11.0;
        }
    }
    y += 6.0;
    b.text(16.0, y, 1.0, "QUEUED ORDERS", DIM);
    y += 12.0;
    if game.portfolio.queued.is_empty() {
        b.text(16.0, y, 1.0, "(none)", DIM);
        y += 12.0;
    } else {
        for o in &game.portfolio.queued {
            let d = &game.market.defs[o.symbol_idx];
            let col = match o.side { crate::sim::portfolio::Side::Buy => GREEN, crate::sim::portfolio::Side::Sell => RED };
            b.text(16.0, y, 1.0, &format!("{:?} {:<5} {:.2}", o.side, d.symbol, o.shares), col);
            y += 11.0;
        }
    }
    y += 8.0;
    b.text(16.0, y, 1.0, "SELECTED", DIM);
    y += 12.0;
    let d = &game.market.defs[ui.selected.min(game.market.defs.len() - 1)];
    let st = &game.market.states[ui.selected.min(game.market.defs.len() - 1)];
    let wk = if st.history.len() >= 2 { st.history[st.history.len() - 1] / st.history[st.history.len() - 2] - 1.0 } else { 0.0 };
    b.text(16.0, y, 1.25, &format!("{} {}", d.symbol, trunc(d.name, 22)), TXT);
    y += 15.0;
    b.text(16.0, y, 1.0, &format!("{} | {}", d.kind.label(), sector_label(d.sector)), DIM);
    y += 11.0;
    b.text(16.0, y, 1.0, &format!("price {}  wk {}", fmt_price(prices[ui.selected.min(prices.len() - 1)]), pct(wk)), if wk >= 0.0 { GREEN } else { RED });
    y += 11.0;
    b.text(16.0, y, 1.0, &format!("you hold {:.2} shares", game.portfolio.shares_of(ui.selected.min(game.market.defs.len() - 1))), TXT);

    // ---------- middle: charts ----------
    b.rect(mid_x, body_top, mid_w, body_bot - body_top, PANEL);
    b.outline(mid_x, body_top, mid_w, body_bot - body_top, EDGE);
    let chart_h = (body_bot - body_top) * 0.58;
    b.text(mid_x + 8.0, body_top + 8.0, 1.25, &format!("{} price (26wk)", d.symbol), ACCENT);
    b.chart(mid_x + 8.0, body_top + 30.0, mid_w - 16.0, chart_h - 44.0, &st.history, ACCENT);
    let lo = st.history.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = st.history.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    b.text(mid_x + 8.0, body_top + 30.0 + chart_h - 44.0 + 4.0, 1.0, &format!("lo {}  hi {}", fmt_price(lo), fmt_price(hi)), DIM);
    let nw_y = body_top + 30.0 + chart_h - 8.0;
    b.text(mid_x + 8.0, nw_y, 1.25, "YOUR NET WORTH", ACCENT);
    b.chart(mid_x + 8.0, nw_y + 20.0, mid_w - 16.0, body_bot - nw_y - 28.0, &game.history_net_worth.iter().map(|&c| c as f64 / 100.0).collect::<Vec<_>>(), GREEN);
    // goal line on net worth chart
    let nw_min = game.history_net_worth.iter().min().copied().unwrap_or(0) as f64 / 100.0;
    let nw_max = game.history_net_worth.iter().max().copied().unwrap_or(1) as f64 / 100.0;
    let nw_r = (nw_max - nw_min).max(1.0);
    let gy = nw_y + 20.0 + (body_bot - nw_y - 28.0) - (1_000_000.0 - nw_min) as f32 / nw_r as f32 * (body_bot - nw_y - 28.0);
    if gy > nw_y + 20.0 && gy < body_bot - 8.0 {
        b.line(mid_x + 8.0, gy, mid_x + mid_w - 8.0, gy, 1.0, YELLOW);
        b.text(mid_x + mid_w - 92.0, gy - 10.0, 1.0, "$1M goal", YELLOW);
    }

    // ---------- right panel: news + tips ----------
    b.rect(right_x, body_top, 432.0, body_bot - body_top, PANEL);
    b.outline(right_x, body_top, 432.0, body_bot - body_top, EDGE);
    let src_name = match ui.news_source {
        Source::Newspaper => "NEWSPAPER: THE DAILY BULL & BEAR  [N cycles]",
        Source::Tv => "TV: MONEY-TV 24  [N cycles]",
        Source::Wire => "WIRE  [N cycles]",
    };
    b.text(right_x + 8.0, body_top + 8.0, 1.1, src_name, ACCENT);
    let mut ny = body_top + 26.0;
    let items = game.news_by(ui.news_source);
    let max_chars = 52;
    for n in items.iter().take(7) {
        let sym = n.symbol.map(|i| game.market.defs[i].symbol).unwrap_or("MKT");
        let arrow = if n.sentiment > 0.25 { "^" } else if n.sentiment < -0.25 { "v" } else { "-" };
        let col = if n.sentiment > 0.25 { GREEN } else if n.sentiment < -0.25 { RED } else { TXT };
        b.text(right_x + 8.0, ny, 1.0, &format!("[{:>4}]{} {}", sym, arrow, trunc(&n.headline, max_chars - 7)), col);
        ny += 11.0;
        b.text(right_x + 8.0, ny, 1.0, &trunc(&n.body, max_chars), DIM);
        ny += 13.0;
    }
    // tips
    let ty = ny + 6.0;
    b.text(right_x + 8.0, ty, 1.1, "TIPS (Tab select, Space follow, X trust)", YELLOW);
    let mut ty = ty + 15.0;
    for (i, t) in game.tips.iter().enumerate().take(4) {
        let p = &game.people[t.person];
        let hl = if i == ui.tip_sel { ACCENT } else { TXT };
        let mark = if p.trusted { "*" } else { " " };
        b.text(right_x + 8.0, ty, 1.0, &format!("{}{} {}: {}", if i == ui.tip_sel { ">" } else { " " }, mark, trunc(&p.name, 12), trunc(&t.text, 34)), hl);
        ty += 11.0;
    }

    // ---------- bottom message / help ----------
    let msg_y = h - ticker_h - bottom_h;
    b.rect(0.0, msg_y, w, bottom_h, PANEL);
    b.rect(0.0, msg_y, w, 1.0, EDGE);
    b.text(10.0, msg_y + 8.0, 1.0, &format!("> {}", trunc(&ui.msg, 150)), YELLOW);
    if ui.show_help {
        b.text(10.0, msg_y + 22.0, 1.0, "1-9,0 select | B buy $1k | SHIFT+B buy max | S sell 10% | SHIFT+S sell all | E end week | Tab/Space tips | X trust | N news source | F FSR | L LLM | H help | Q quit", DIM);
    } else {
        b.text(10.0, msg_y + 22.0, 1.0, "keys: 1-9 select, B/S trade, E end week, Tab+Space tips, N news, F FSR, L LLM, H help, Q quit   |   status: trade once per week; expenses $1,500/wk; reach $1M in 26 weeks", DIM);
    }
    let status = match game.status {
        Status::Playing => "PLAYING",
        Status::Won => "HIRED BY A TRADING FIRM — YOU WIN",
        Status::Bankrupt => "BANKRUPT — PENNILESS",
        Status::Failed => "UNDER $1M AFTER 26 WEEKS — PENNILESS",
    };
    let scol = match game.status {
        Status::Playing => TXT,
        Status::Won => GREEN,
        _ => RED,
    };
    b.text(10.0, msg_y + 40.0, 1.25, status, scol);

    // ---------- ticker (bottom) ----------
    let ty = h - ticker_h;
    b.rect(0.0, ty, w, ticker_h, TICKER_BG);
    b.rect(0.0, ty, w, 1.0, EDGE);
    b.text(6.0, ty + 4.0, 1.0, "LIVE", RED);
    // scrolling ticker text built from all instruments
    let mut segs: Vec<(String, [f32; 4])> = Vec::new();
    for (i, d) in game.market.defs.iter().enumerate() {
        let st = &game.market.states[i];
        let wk = if st.history.len() >= 2 { st.history[st.history.len() - 1] / st.history[st.history.len() - 2] - 1.0 } else { 0.0 };
        let col = if wk >= 0.0 { GREEN } else { RED };
        segs.push((format!("{} {} {}   ", d.symbol, fmt_price(prices[i]), pct(wk)), col));
    }
    let full: String = segs.iter().map(|(s, _)| s.clone()).collect();
    let full_w = full.chars().count() as f32 * FONT_W;
    let mut offset = (ui.frame as f32 * 1.5) % full_w.max(1.0);
    if offset < 0.0 {
        offset += full_w;
    }
    // draw twice for seamless wrap
    for pass in 0..2 {
        let mut x = 44.0 - offset + pass as f32 * full_w;
        for (s, col) in &segs {
            let wpx = b.text(x, ty + 9.0, 1.0, s, *col);
            x += wpx;
        }
    }

    b
}

fn sector_label(s: crate::sim::instruments::Sector) -> &'static str {
    s.label()
}
