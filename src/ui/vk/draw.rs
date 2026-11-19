//! CPU-side immediate-mode quad/triangle batch + the full game UI layout.
//! Uses custom pixel-art/vector illustrations (`super::art`), smooth procedural
//! shader primitives (circles, glows, pills, scanlines), and multi-stop color
//! gradients for a rich Wall Street arcade aesthetic.

use crate::sim::game::{Game, Status, TOTAL_WEEKS};
use crate::sim::instruments::{is_earnings_week, Kind};
use crate::sim::market::Regime;
use crate::sim::news::Source;
use crate::sim::portfolio::{fmt_money, fmt_price, Side};
use super::art;

pub const FONT_W: f32 = 8.0;
pub const FONT_H: f32 = 8.0;
pub const LOGICAL_W: f32 = 1280.0;
pub const LOGICAL_H: f32 = 720.0;

// Rich vibrant palette
pub const BG_TOP: [f32; 4] = [0.035, 0.050, 0.105, 1.0];
pub const BG_BOT: [f32; 4] = [0.018, 0.024, 0.052, 1.0];
pub const PANEL_TOP: [f32; 4] = [0.075, 0.100, 0.165, 0.96];
pub const PANEL_BOT: [f32; 4] = [0.042, 0.056, 0.098, 0.98];
pub const PANEL2: [f32; 4] = [0.10, 0.135, 0.21, 1.0];
pub const EDGE: [f32; 4] = [0.20, 0.28, 0.42, 1.0];
pub const EDGE_HI: [f32; 4] = [0.32, 0.48, 0.72, 1.0];
pub const TXT: [f32; 4] = [0.92, 0.95, 0.99, 1.0];
pub const DIM: [f32; 4] = [0.52, 0.60, 0.74, 1.0];
pub const ACCENT: [f32; 4] = [0.22, 0.68, 1.00, 1.0];
pub const CYAN: [f32; 4] = [0.15, 0.90, 0.98, 1.0];
pub const GREEN: [f32; 4] = [0.16, 0.92, 0.48, 1.0];
pub const GREEN_DARK: [f32; 4] = [0.06, 0.32, 0.18, 1.0];
pub const RED: [f32; 4] = [0.98, 0.28, 0.30, 1.0];
pub const RED_DARK: [f32; 4] = [0.36, 0.08, 0.10, 1.0];
pub const YELLOW: [f32; 4] = [0.98, 0.80, 0.20, 1.0];
pub const GOLD_DARK: [f32; 4] = [0.38, 0.28, 0.06, 1.0];
pub const PURPLE: [f32; 4] = [0.72, 0.38, 0.98, 1.0];
pub const ORANGE: [f32; 4] = [1.00, 0.56, 0.18, 1.0];
pub const TICKER_BG: [f32; 4] = [0.015, 0.022, 0.042, 1.0];

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
            msg: "Welcome to Wall Street! Queue BUY/SELL orders, then press E to close the week.".to_string(),
            show_help: false,
            frame: 0,
            fsr: true,
            fsr_scale: 1.0,
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

    #[inline]
    fn push_v(&mut self, x: f32, y: f32, col: [f32; 4], u: f32, w: f32, mode: f32) {
        self.v.extend_from_slice(&[x, y, col[0], col[1], col[2], col[3], u, w, mode, 0.0]);
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
            self.push_v(x, y, col, u, w, mode);
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.quad(x, y, x + w, y + h, col, (0.0, 0.0), (0.0, 0.0), 0.0);
    }

    /// Vertical gradient rectangle.
    pub fn rect_grad_v(&mut self, x: f32, y: f32, w: f32, h: f32, top: [f32; 4], bot: [f32; 4]) {
        let (x0, y0, x1, y1) = (x, y, x + w, y + h);
        self.push_v(x0, y0, top, 0.0, 0.0, 0.0);
        self.push_v(x1, y0, top, 0.0, 0.0, 0.0);
        self.push_v(x0, y1, bot, 0.0, 0.0, 0.0);
        self.push_v(x0, y1, bot, 0.0, 0.0, 0.0);
        self.push_v(x1, y0, top, 0.0, 0.0, 0.0);
        self.push_v(x1, y1, bot, 0.0, 0.0, 0.0);
    }

    /// Horizontal gradient rectangle.
    pub fn rect_grad_h(&mut self, x: f32, y: f32, w: f32, h: f32, left: [f32; 4], right: [f32; 4]) {
        let (x0, y0, x1, y1) = (x, y, x + w, y + h);
        self.push_v(x0, y0, left, 0.0, 0.0, 0.0);
        self.push_v(x1, y0, right, 0.0, 0.0, 0.0);
        self.push_v(x0, y1, left, 0.0, 0.0, 0.0);
        self.push_v(x0, y1, left, 0.0, 0.0, 0.0);
        self.push_v(x1, y0, right, 0.0, 0.0, 0.0);
        self.push_v(x1, y1, right, 0.0, 0.0, 0.0);
    }

    /// Arbitrary colored triangle.
    pub fn tri_grad(&mut self, p0: (f32, f32, [f32; 4]), p1: (f32, f32, [f32; 4]), p2: (f32, f32, [f32; 4])) {
        self.push_v(p0.0, p0.1, p0.2, 0.0, 0.0, 0.0);
        self.push_v(p1.0, p1.1, p1.2, 0.0, 0.0, 0.0);
        self.push_v(p2.0, p2.1, p2.2, 0.0, 0.0, 0.0);
    }

    /// Anti-aliased circle using fragment shader mode=2.0.
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
        self.quad(cx - r, cy - r, cx + r, cy + r, col, (-1.0, -1.0), (1.0, 1.0), 2.0);
    }

    /// Soft radial spotlight glow using fragment shader mode=3.0.
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
        self.quad(cx - r, cy - r, cx + r, cy + r, col, (-1.0, -1.0), (1.0, 1.0), 3.0);
    }

    /// Rounded pill badge using fragment shader mode=4.0.
    pub fn pill(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.quad(x, y, x + w, y + h, col, (-1.0, -1.0), (1.0, 1.0), 4.0);
    }

    pub fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.rect(x, y, w, 1.0, col);
        self.rect(x, y + h - 1.0, w, 1.0, col);
        self.rect(x, y, 1.0, h, col);
        self.rect(x + w - 1.0, y, 1.0, h, col);
    }

    /// Styled panel card with drop shadow, gradient body, colored header bar, and corner accents.
    pub fn panel_card(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        title: &str,
        accent: [f32; 4],
    ) {
        // Drop shadow
        self.rect(x + 3.0, y + 3.0, w, h, [0.0, 0.0, 0.0, 0.45]);
        // Main card gradient
        self.rect_grad_v(x, y, w, h, PANEL_TOP, PANEL_BOT);
        // Header gradient strip
        let hh = 22.0;
        self.rect_grad_h(
            x + 1.0,
            y + 1.0,
            w - 2.0,
            hh,
            [accent[0] * 0.38, accent[1] * 0.38, accent[2] * 0.38, 0.95],
            [0.08, 0.11, 0.18, 0.95],
        );
        // Header left accent bar + bottom divider
        self.rect(x + 1.0, y + 1.0, 4.0, hh, accent);
        self.rect_grad_h(x + 1.0, y + hh, w - 2.0, 1.0, accent, EDGE);
        // Outer border & bright corner ticks
        self.outline(x, y, w, h, EDGE);
        let c_len = 6.0;
        self.rect(x, y, c_len, 1.5, accent);
        self.rect(x, y, 1.5, c_len, accent);
        self.rect(x + w - c_len, y, c_len, 1.5, accent);
        self.rect(x + w - 1.5, y, 1.5, c_len, accent);
        self.rect(x, y + h - 1.5, c_len, 1.5, accent);
        self.rect(x + w - c_len, y + h - 1.5, c_len, 1.5, accent);

        if !title.is_empty() {
            self.text_shadow(x + 10.0, y + 6.0, 1.15, title, TXT);
        }
    }

    /// Draw text. Maps custom symbols (▲, ▼, ◆, •, ★, ●, —, ⇧) to slots 1..8 in FONT8X8.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, s: &str, col: [f32; 4]) -> f32 {
        let mut cx = x.round();
        let mut cy = y.round();
        let x0 = cx;
        for ch in s.chars() {
            if ch == '\n' {
                cx = x0;
                cy = (cy + FONT_H * scale).round();
                continue;
            }
            let code = match ch {
                '▲' | '↑' => 1,
                '▼' | '↓' => 2,
                '◆' => 3,
                '•' | '·' => 4,
                '★' => 5,
                '●' => 6,
                '—' | '–' => 7,
                '⇧' => 8,
                c if (c as u32) < 128 => c as usize,
                _ => b'?' as usize,
            };
            let u0 = ((code % 16) * 8) as f32;
            let v0 = ((code / 16) * 8) as f32;
            let w = FONT_W * scale;
            let h = FONT_H * scale;
            self.quad(cx, cy, cx + w, cy + h, col, (u0, v0), (u0 + 8.0, v0 + 8.0), 1.0);
            cx += w;
        }
        cx - x0
    }

    /// Draw text with a 1px dark drop shadow for high contrast.
    pub fn text_shadow(&mut self, x: f32, y: f32, scale: f32, s: &str, col: [f32; 4]) -> f32 {
        self.text(x + 1.0, y + 1.0, scale, s, [0.0, 0.0, 0.0, 0.78]);
        self.text(x, y, scale, s, col)
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
            self.push_v(x, y, col, 0.0, 0.0, 0.0);
        }
    }

    /// Rich financial chart with gridlines, gradient area fill, delta bars, glowing line & nodes.
    pub fn chart_pro(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        data: &[f64],
        col: [f32; 4],
        ref_val: Option<f64>,
        show_bars: bool,
    ) {
        // Recessed dark chart canvas
        self.rect_grad_v(x, y, w, h, [0.025, 0.036, 0.068, 0.95], [0.015, 0.022, 0.042, 0.95]);
        self.outline(x, y, w, h, [0.14, 0.20, 0.32, 1.0]);

        // Subtle horizontal & vertical grid lines
        for g in 1..4 {
            let gy = y + h * (g as f32 / 4.0);
            self.rect(x + 1.0, gy, w - 2.0, 1.0, [0.12, 0.17, 0.28, 0.55]);
        }
        for gx_i in 1..8 {
            let gx = x + w * (gx_i as f32 / 8.0);
            self.rect(gx, y + 1.0, 1.0, h - 2.0, [0.10, 0.15, 0.25, 0.40]);
        }

        if data.is_empty() {
            return;
        }
        let mut min = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let mut max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if let Some(rv) = ref_val {
            min = min.min(rv);
            max = max.max(rv);
        }
        let pad = ((max - min) * 0.08).max(1e-6);
        min -= pad;
        max += pad;
        let range = (max - min).max(1e-9);

        let pad_top = 6.0;
        let pad_bot = if show_bars { 22.0 } else { 6.0 };
        let plot_y = y + pad_top;
        let plot_h = (h - pad_top - pad_bot).max(10.0);

        let n = data.len();
        let px = |i: usize| {
            if n <= 1 {
                x + w * 0.5
            } else {
                x + 6.0 + (w - 12.0) * i as f32 / (n - 1) as f32
            }
        };
        let py = |v: f64| plot_y + plot_h - ((v - min) / range) as f32 * plot_h;

        // Optional reference baseline (dashed)
        if let Some(rv) = ref_val {
            let ry = py(rv);
            if ry >= plot_y && ry <= plot_y + plot_h {
                let mut dx = x + 4.0;
                while dx + 6.0 < x + w - 4.0 {
                    self.rect(dx, ry, 4.0, 1.0, [0.55, 0.65, 0.82, 0.55]);
                    dx += 8.0;
                }
            }
        }

        if n >= 2 {
            let fill_top = [col[0], col[1], col[2], 0.34];
            let fill_bot = [col[0] * 0.4, col[1] * 0.4, col[2] * 0.4, 0.02];
            let base_y = plot_y + plot_h;

            // Shaded gradient area under curve
            for i in 0..n - 1 {
                let (x0, y0) = (px(i), py(data[i]));
                let (x1, y1) = (px(i + 1), py(data[i + 1]));
                self.tri_grad((x0, y0, fill_top), (x1, y1, fill_top), (x0, base_y, fill_bot));
                self.tri_grad((x0, base_y, fill_bot), (x1, y1, fill_top), (x1, base_y, fill_bot));
            }

            // Weekly delta histogram bars at the bottom of the chart
            if show_bars {
                let bar_zone_y = y + h - 18.0;
                self.rect(x + 1.0, bar_zone_y - 2.0, w - 2.0, 1.0, [0.14, 0.20, 0.32, 0.7]);
                let step_w = ((w - 16.0) / n as f32).clamp(3.0, 14.0);
                for i in 1..n {
                    let ret = (data[i] / data[i - 1].max(1e-9)) - 1.0;
                    let bh = (ret.abs() * 140.0).clamp(2.0, 14.0) as f32;
                    let bcol = if ret >= 0.0 {
                        [GREEN[0], GREEN[1], GREEN[2], 0.78]
                    } else {
                        [RED[0], RED[1], RED[2], 0.78]
                    };
                    let bx = px(i) - step_w * 0.4;
                    self.rect(bx, y + h - 2.0 - bh, (step_w * 0.8).max(2.0), bh, bcol);
                }
            }

            // Outer glow line + bright core line
            let glow_col = [col[0], col[1], col[2], 0.28];
            for i in 0..n - 1 {
                self.line(px(i), py(data[i]), px(i + 1), py(data[i + 1]), 4.5, glow_col);
            }
            for i in 0..n - 1 {
                self.line(px(i), py(data[i]), px(i + 1), py(data[i + 1]), 2.0, col);
            }

            // Data point nodes + glowing beacon on latest point
            for i in 0..n {
                let (cx, cy) = (px(i), py(data[i]));
                if i + 1 == n {
                    self.glow(cx, cy, 10.0, [col[0], col[1], col[2], 0.65]);
                    self.circle(cx, cy, 3.8, TXT);
                    self.circle(cx, cy, 2.2, col);
                } else {
                    self.circle(cx, cy, 2.0, col);
                }
            }
        }
    }

    /// Mini sparkline inside a small rectangle (for watchlist rows).
    pub fn mini_spark(&mut self, x: f32, y: f32, w: f32, h: f32, data: &[f64], col: [f32; 4]) {
        let start = data.len().saturating_sub(8);
        let tail = &data[start..];
        if tail.len() < 2 {
            return;
        }
        let min = tail.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = tail.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = (max - min).max(1e-9);
        let n = tail.len();
        for i in 0..n - 1 {
            let x0 = x + w * (i as f32 / (n - 1) as f32);
            let x1 = x + w * ((i + 1) as f32 / (n - 1) as f32);
            let y0 = y + h - ((tail[i] - min) / range) as f32 * h;
            let y1 = y + h - ((tail[i + 1] - min) / range) as f32 * h;
            self.line(x0, y0, x1, y1, 1.4, col);
        }
    }
}

fn trunc(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        s.to_string()
    } else {
        format!("{}..", s.chars().take(max_chars.saturating_sub(2)).collect::<String>())
    }
}

fn pct(x: f64) -> String {
    format!("{:+.2}%", x * 100.0)
}

fn kind_badge_info(k: Kind) -> (&'static str, [f32; 4]) {
    match k {
        Kind::Stock => ("STK", [0.20, 0.58, 0.98, 1.0]),
        Kind::Etf => ("ETF", [0.16, 0.78, 0.76, 1.0]),
        Kind::CryptoEtf => ("CET", [0.68, 0.38, 0.98, 1.0]),
        Kind::Crypto => ("CRY", [0.98, 0.56, 0.16, 1.0]),
    }
}

/// Build the entire frame's geometry in the logical 1280x720 coordinate space
/// (scaled by the GPU viewport to any window / fullscreen resolution).
pub fn build(game: &Game, ui: &UiState, _w: f32, _h: f32) -> Batch {
    let (w, h) = (LOGICAL_W, LOGICAL_H);
    let mut b = Batch::new();
    let prices = game.prices();
    let nw = game.net_worth_cents();
    let goal_frac = (nw as f64 / 1_000_000_00.0).clamp(0.0, 1.0) as f32;

    // ---------- atmospheric background ----------
    b.rect_grad_v(0.0, 0.0, w, h, BG_TOP, BG_BOT);
    b.glow(w * 0.25, h * 0.20, 320.0, [0.12, 0.32, 0.68, 0.14]);
    b.glow(w * 0.78, h * 0.35, 340.0, [0.42, 0.14, 0.58, 0.12]);

    let top_h = 58.0;
    let ticker_h = 28.0;
    let deck_h = 54.0;

    // =========================================================================
    // 1. TOP BANNER: Skyline + Bull & Bear Mascots + KPI Cards + Trophy Goal
    // =========================================================================
    b.rect_grad_v(0.0, 0.0, w, top_h, [0.08, 0.12, 0.24, 1.0], [0.04, 0.06, 0.12, 1.0]);
    art::draw_skyline(&mut b, 0.0, 2.0, w, top_h - 3.0, ui.frame);
    b.rect_grad_h(0.0, top_h - 2.0, w, 2.0, ACCENT, YELLOW);

    // Bull & Bear Mascots + Title
    b.glow(36.0, 28.0, 28.0, [0.98, 0.78, 0.18, 0.32]);
    art::draw_bull(&mut b, 10.0, 12.0, 2.2);
    b.text_shadow(60.0, 9.0, 1.75, "WALL STREET CHAMPION", YELLOW);
    b.text_shadow(60.0, 27.0, 0.95, "TURN $100,000 INTO $1,000,000 IN 26 WEEKS", CYAN);

    // Week timeline mini-bar under title
    let tl_x = 60.0;
    let tl_y = 41.0;
    let cur_wk = game.week.min(TOTAL_WEEKS);
    b.text_shadow(tl_x, tl_y, 1.0, &format!("WK {:>2}/{}", cur_wk, TOTAL_WEEKS), TXT);
    for wk in 1..=TOTAL_WEEKS {
        let sx = tl_x + 74.0 + (wk - 1) as f32 * 9.0;
        let is_earn = is_earnings_week(wk);
        let col = if wk < cur_wk {
            GREEN
        } else if wk == cur_wk {
            YELLOW
        } else if is_earn {
            [0.45, 0.35, 0.68, 0.9]
        } else {
            [0.16, 0.22, 0.34, 0.9]
        };
        let sh = if is_earn { 9.0 } else { 6.0 };
        b.rect(sx, tl_y + (9.0 - sh) * 0.5, 6.5, sh, col);
    }
    art::draw_bear(&mut b, 382.0, 14.0, 2.2);

    // Center KPI Cards (Cash, Debt, Net Worth, Market Regime)
    let kpi_x = 436.0;
    let draw_kpi = |b: &mut Batch, x: f32, w_box: f32, label: &str, val: &str, vcol: [f32; 4]| {
        b.rect_grad_v(x, 8.0, w_box, 42.0, [0.09, 0.13, 0.22, 0.92], [0.05, 0.07, 0.13, 0.92]);
        b.rect(x, 8.0, w_box, 2.0, vcol);
        b.outline(x, 8.0, w_box, 42.0, EDGE);
        b.text(x + 7.0, 14.0, 0.90, label, DIM);
        b.text_shadow(x + 7.0, 28.0, 1.15, val, vcol);
    };
    draw_kpi(&mut b, kpi_x, 122.0, "CASH BALANCE", &fmt_money(game.portfolio.cash_cents), CYAN);
    let debt = game.portfolio.debt_cents();
    draw_kpi(
        &mut b,
        kpi_x + 128.0,
        114.0,
        "MARGIN DEBT",
        &fmt_money(debt),
        if debt > 0 { ORANGE } else { DIM },
    );
    draw_kpi(
        &mut b,
        kpi_x + 248.0,
        134.0,
        "NET WORTH",
        &fmt_money(nw),
        if nw >= 100_000_00 { GREEN } else { RED },
    );

    // Market Regime Badge
    let (reg_str, reg_col) = match game.market.regime {
        Regime::Bull => ("BULL MKT ▲", GREEN),
        Regime::Bear => ("BEAR MKT ▼", RED),
        Regime::Sideways => ("SIDEWAYS ◆", YELLOW),
    };
    draw_kpi(&mut b, kpi_x + 388.0, 106.0, "REGIME", reg_str, reg_col);

    // Right: Trophy + $1,000,000 Goal Progress Box
    let goal_x = w - 332.0;
    let goal_w = 324.0;
    b.rect_grad_v(goal_x, 8.0, goal_w, 42.0, [0.10, 0.14, 0.24, 0.94], [0.05, 0.08, 0.14, 0.94]);
    b.outline(goal_x, 8.0, goal_w, 42.0, [0.55, 0.46, 0.18, 1.0]);
    art::draw_trophy(&mut b, goal_x + 8.0, 12.0, 2.6);

    let bx = goal_x + 46.0;
    let bw = goal_w - 54.0;
    b.text_shadow(
        bx,
        12.0,
        1.0,
        &format!("$1M GOAL: {:>5.1}%", goal_frac * 100.0),
        YELLOW,
    );
    b.pill(bx + bw - 104.0, 10.0, 104.0, 12.0, [0.14, 0.20, 0.34, 1.0]);
    b.text(
        bx + bw - 98.0,
        12.0,
        0.85,
        &format!("FSR {}  LLM {}", if ui.fsr { "ON" } else { "OFF" }, if ui.llm_on { "ON" } else { "OFF" }),
        CYAN,
    );

    // Gradient goal progress bar with quarter ticks
    b.rect(bx, 26.0, bw, 14.0, [0.04, 0.06, 0.11, 1.0]);
    let fill_w = (bw * goal_frac).max(2.0);
    b.rect_grad_h(bx, 26.0, fill_w, 14.0, ACCENT, if nw >= 1_000_000_00 { GREEN } else { YELLOW });
    for q in 1..4 {
        let qx = bx + bw * (q as f32 * 0.25);
        b.rect(qx, 26.0, 1.0, 14.0, [0.85, 0.90, 1.0, 0.35]);
    }
    b.outline(bx, 26.0, bw, 14.0, EDGE_HI);

    // =========================================================================
    // 2. BODY LAYOUT (3 COLUMNS)
    // =========================================================================
    let body_top = top_h + 6.0;
    let body_bot = h - ticker_h - deck_h - 6.0;
    let left_x = 8.0;
    let left_w = 356.0;
    let mid_x = left_x + left_w + 8.0;
    let right_w = 436.0;
    let right_x = w - right_w - 8.0;
    let mid_w = right_x - mid_x - 8.0;

    // -------------------------------------------------------------------------
    // LEFT COLUMN TOP: PORTFOLIO & ASSET ALLOCATION
    // -------------------------------------------------------------------------
    let port_h = 196.0;
    b.panel_card(left_x, body_top, left_w, port_h, "PORTFOLIO & ALLOCATION", GREEN);

    let mut py = body_top + 28.0;
    let bp = game.portfolio.buying_power_cents(&prices).max(0);
    let tot_ret = (nw as f64 / 100_000_00.0 - 1.0) * 100.0;
    b.text(left_x + 10.0, py, 1.0, &format!("Buying Power: {}", fmt_money(bp)), TXT);
    let ret_col = if tot_ret >= 0.0 { GREEN } else { RED };
    b.text_shadow(left_x + 228.0, py, 1.0, &format!("P/L {:+.1}%", tot_ret), ret_col);
    py += 14.0;

    // Compute allocation across Stocks, ETFs, Crypto, Cash
    let mut val_stk = 0.0f64;
    let mut val_etf = 0.0f64;
    let mut val_cry = 0.0f64;
    for (&i, &sh) in &game.portfolio.holdings {
        let v = (sh * prices[i] * 100.0).max(0.0);
        match game.market.defs[i].kind {
            Kind::Stock => val_stk += v,
            Kind::Etf => val_etf += v,
            Kind::CryptoEtf | Kind::Crypto => val_cry += v,
        }
    }
    let val_cash = (game.portfolio.cash_cents.max(0)) as f64;
    let denom = (val_stk + val_etf + val_cry + val_cash).max(1.0);
    let bar_x = left_x + 10.0;
    let bar_w = left_w - 20.0;
    b.rect(bar_x, py, bar_w, 10.0, [0.04, 0.06, 0.11, 1.0]);
    let mut cur_bx = bar_x;
    for (v, col) in [
        (val_stk, ACCENT),
        (val_etf, CYAN),
        (val_cry, ORANGE),
        (val_cash, GREEN),
    ] {
        let sw = (bar_w as f64 * (v / denom)) as f32;
        if sw > 0.5 {
            b.rect(cur_bx, py, sw, 10.0, col);
            cur_bx += sw;
        }
    }
    b.outline(bar_x, py, bar_w, 10.0, EDGE);
    py += 13.0;

    // Legend row
    let leg = [
        ("STK", ACCENT),
        ("ETF", CYAN),
        ("CRYPTO", ORANGE),
        ("CASH", GREEN),
    ];
    let mut lx = left_x + 10.0;
    for (lbl, col) in leg {
        b.rect(lx, py + 1.0, 6.0, 6.0, col);
        b.text(lx + 9.0, py, 0.88, lbl, DIM);
        lx += 82.0;
    }
    py += 13.0;

    // Holdings & Queued orders side-by-side inside Portfolio card
    b.rect(left_x + 8.0, py, left_w - 16.0, 1.0, EDGE);
    py += 4.0;
    let col2_x = left_x + 196.0;
    b.text(left_x + 10.0, py, 0.95, "ACTIVE HOLDINGS", CYAN);
    b.text(col2_x, py, 0.95, "QUEUED ORDERS", YELLOW);
    py += 12.0;

    let mut hy = py;
    if game.portfolio.holdings.is_empty() {
        b.text(left_x + 10.0, hy, 0.92, "(none — 100% cash)", DIM);
    } else {
        let mut rows: Vec<(usize, f64)> = game.portfolio.holdings.iter().map(|(&i, &s)| (i, s)).collect();
        rows.sort_by_key(|&(i, _)| i);
        for (i, sh) in rows.into_iter().take(8) {
            let d = &game.market.defs[i];
            let val = (sh * prices[i] * 100.0) as i64;
            let hl = if i == ui.selected { YELLOW } else { TXT };
            b.text(
                left_x + 10.0,
                hy,
                0.92,
                &format!("{:<4} {:>6.1} {}", d.symbol, sh, fmt_money(val)),
                hl,
            );
            hy += 11.5;
        }
    }

    let mut qy = py;
    if game.portfolio.queued.is_empty() {
        b.text(col2_x, qy, 0.92, "(no orders queued)", DIM);
    } else {
        for o in game.portfolio.queued.iter().take(8) {
            let d = &game.market.defs[o.symbol_idx];
            let (tag, col) = match o.side {
                Side::Buy => ("BUY ", GREEN),
                Side::Sell => ("SELL", RED),
            };
            b.text(col2_x, qy, 0.92, &format!("{} {:<4} {:.2}sh", tag, d.symbol, o.shares), col);
            qy += 11.5;
        }
    }

    // -------------------------------------------------------------------------
    // LEFT COLUMN BOTTOM: INTERACTIVE MARKET WATCHLIST (ALL 23 INSTRUMENTS)
    // -------------------------------------------------------------------------
    let wl_top = body_top + port_h + 6.0;
    let wl_h = body_bot - wl_top;
    b.panel_card(left_x, wl_top, left_w, wl_h, "MARKET WATCHLIST  [↑/↓ or 1-0]", ACCENT);

    let n_inst = game.market.defs.len();
    let sel = ui.selected.min(n_inst.saturating_sub(1));
    let avail_h = wl_h - 26.0;
    let row_h = (avail_h / n_inst as f32).clamp(11.0, 15.0);
    let mut wy = wl_top + 24.0;

    for (i, d) in game.market.defs.iter().enumerate() {
        let st = &game.market.states[i];
        let wk = if st.history.len() >= 2 {
            st.history[st.history.len() - 1] / st.history[st.history.len() - 2] - 1.0
        } else {
            0.0
        };
        let is_sel = i == sel;
        let held = game.portfolio.shares_of(i) > 0.0;

        if is_sel {
            b.rect_grad_h(
                left_x + 3.0,
                wy - 0.5,
                left_w - 6.0,
                row_h,
                [0.18, 0.42, 0.78, 0.85],
                [0.08, 0.18, 0.36, 0.85],
            );
            b.rect(left_x + 3.0, wy - 0.5, 3.0, row_h, YELLOW);
        } else if i % 2 == 1 {
            b.rect(left_x + 3.0, wy - 0.5, left_w - 6.0, row_h, [0.06, 0.08, 0.14, 0.55]);
        }

        let (k_lbl, k_col) = kind_badge_info(d.kind);
        b.rect(left_x + 8.0, wy + 1.0, 24.0, row_h - 2.5, [k_col[0] * 0.35, k_col[1] * 0.35, k_col[2] * 0.35, 0.95]);
        b.text(left_x + 10.0, wy + 2.0, 0.82, k_lbl, k_col);

        let sym_col = if is_sel { YELLOW } else if held { CYAN } else { TXT };
        let hold_mark = if held { "★" } else { " " };
        b.text(left_x + 35.0, wy + 2.0, 0.95, &format!("{}{:<4}", hold_mark, d.symbol), sym_col);
        b.text(left_x + 84.0, wy + 2.0, 0.85, &trunc(d.name, 14), if is_sel { TXT } else { DIM });
        b.text(left_x + 190.0, wy + 2.0, 0.92, &format!("{:>8}", fmt_price(prices[i])), TXT);

        let chg_col = if wk >= 0.0 { GREEN } else { RED };
        let arrow = if wk >= 0.0 { "▲" } else { "▼" };
        b.text(left_x + 258.0, wy + 2.0, 0.90, &format!("{}{:>6}", arrow, pct(wk)), chg_col);

        b.mini_spark(left_x + 318.0, wy + 2.0, 30.0, row_h - 4.0, &st.history, chg_col);
        wy += row_h;
    }

    // -------------------------------------------------------------------------
    // MIDDLE COLUMN: SELECTED INSTRUMENT SHOWCASE + 2 PRO CHARTS
    // -------------------------------------------------------------------------
    let d = &game.market.defs[sel];
    let st = &game.market.states[sel];
    let wk = if st.history.len() >= 2 {
        st.history[st.history.len() - 1] / st.history[st.history.len() - 2] - 1.0
    } else {
        0.0
    };

    // 1) Selected Instrument Showcase Card
    let info_h = 92.0;
    b.panel_card(mid_x, body_top, mid_w, info_h, "SELECTED INSTRUMENT", CYAN);
    art::draw_asset_emblem(&mut b, mid_x + 10.0, body_top + 28.0, 54.0, d.kind, d.sector);

    let ix = mid_x + 72.0;
    b.text_shadow(ix, body_top + 28.0, 1.35, &format!("{} — {}", d.symbol, trunc(d.name, 16)), TXT);

    let (_, k_col) = kind_badge_info(d.kind);
    b.pill(ix, body_top + 46.0, 86.0, 14.0, [k_col[0] * 0.35, k_col[1] * 0.35, k_col[2] * 0.35, 1.0]);
    b.text(ix + 7.0, body_top + 49.0, 0.90, d.kind.label(), k_col);

    b.pill(ix + 92.0, body_top + 46.0, 128.0, 14.0, [0.14, 0.22, 0.36, 1.0]);
    b.text(ix + 100.0, body_top + 49.0, 0.90, &d.sector.label().to_uppercase(), CYAN);

    // Price & Weekly Change Callout on Right of Showcase Card
    let px_col = if wk >= 0.0 { GREEN } else { RED };
    let px_box_x = mid_x + mid_w - 138.0;
    b.rect_grad_v(
        px_box_x,
        body_top + 27.0,
        130.0,
        36.0,
        if wk >= 0.0 { GREEN_DARK } else { RED_DARK },
        [0.04, 0.06, 0.12, 1.0],
    );
    b.outline(px_box_x, body_top + 27.0, 130.0, 36.0, px_col);
    b.text_shadow(px_box_x + 8.0, body_top + 31.0, 1.35, &fmt_price(prices[sel]), TXT);
    b.text_shadow(
        px_box_x + 8.0,
        body_top + 47.0,
        1.0,
        &format!("{} {} wk", if wk >= 0.0 { "▲" } else { "▼" }, pct(wk)),
        px_col,
    );

    // Volatility meter + Dividend yield + Holdings row
    let vy = body_top + 68.0;
    b.text(ix, vy, 0.92, "VOL:", DIM);
    let vol_bars = ((d.weekly_vol / 0.03).ceil() as usize).clamp(1, 5);
    for vb in 0..5 {
        let vcol = if vb < vol_bars {
            match vb {
                0 | 1 => GREEN,
                2 => YELLOW,
                3 => ORANGE,
                _ => RED,
            }
        } else {
            [0.14, 0.18, 0.28, 1.0]
        };
        b.rect(ix + 34.0 + vb as f32 * 8.5, vy + 1.0, 6.5, 7.0, vcol);
    }
    let div_str = if d.div_yield > 0.0 {
        format!("DIV {:.1}%", d.div_yield * 100.0)
    } else {
        "DIV —".to_string()
    };
    b.text(ix + 82.0, vy, 0.90, &format!("{}  BETA {:+.2}", div_str, d.beta), DIM);
    let held_sh = game.portfolio.shares_of(sel);
    let held_val = (held_sh * prices[sel] * 100.0) as i64;
    b.text_shadow(
        ix + 216.0,
        vy,
        0.90,
        &format!("HELD {:.1}sh ({})", held_sh, fmt_money(held_val)),
        if held_sh > 0.0 { YELLOW } else { DIM },
    );

    // 2) Primary 26-Week Price Action Chart
    let pchart_top = body_top + info_h + 6.0;
    let pchart_h = (body_bot - pchart_top) * 0.54;
    b.panel_card(
        mid_x,
        pchart_top,
        mid_w,
        pchart_h,
        &format!("{} — 26-WK PRICE & DELTA", d.symbol),
        ACCENT,
    );

    let lo = st.history.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = st.history.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let start_p = st.history.first().copied().unwrap_or(d.start_price);
    let overall_up = prices[sel] >= start_p;
    let curve_col = if overall_up { CYAN } else { ORANGE };

    b.text(
        mid_x + mid_w - 196.0,
        pchart_top + 7.0,
        0.90,
        &format!("HI {}  LO {}", fmt_price(hi), fmt_price(lo)),
        DIM,
    );
    b.chart_pro(
        mid_x + 8.0,
        pchart_top + 26.0,
        mid_w - 16.0,
        pchart_h - 34.0,
        &st.history,
        curve_col,
        Some(start_p),
        true,
    );

    // Overlay Y-axis High / Low tags inside top-left and bottom-left of price chart
    b.text_shadow(mid_x + 14.0, pchart_top + 30.0, 0.88, &format!("Max {}", fmt_price(hi)), DIM);
    b.text_shadow(
        mid_x + 14.0,
        pchart_top + pchart_h - 36.0,
        0.88,
        &format!("Min {}", fmt_price(lo)),
        DIM,
    );

    // 3) Net Worth Trajectory Chart
    let nw_top = pchart_top + pchart_h + 6.0;
    let nw_h = body_bot - nw_top;
    b.panel_card(
        mid_x,
        nw_top,
        mid_w,
        nw_h,
        "NET WORTH ($100K -> $1M)",
        GREEN,
    );
    b.text(
        mid_x + mid_w - 204.0,
        nw_top + 7.0,
        0.88,
        "Exp -$1.5k/wk • Margin 0.5%",
        YELLOW,
    );

    let nw_dollars: Vec<f64> = game.history_net_worth.iter().map(|&c| c as f64 / 100.0).collect();
    let nw_col = if nw >= 100_000_00 { GREEN } else { RED };
    let nw_chart_y = nw_top + 26.0;
    let nw_chart_h = nw_h - 34.0;
    b.chart_pro(
        mid_x + 8.0,
        nw_chart_y,
        mid_w - 16.0,
        nw_chart_h,
        &nw_dollars,
        nw_col,
        Some(100_000.0),
        false,
    );

    // $100k start label & peak label
    let nw_max = nw_dollars.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(100_000.0);
    b.text_shadow(mid_x + 14.0, nw_chart_y + 4.0, 0.88, &format!("Peak ${:.0}k", nw_max / 1000.0), GREEN);
    b.text_shadow(
        mid_x + 14.0,
        nw_chart_y + nw_chart_h - 12.0,
        0.88,
        "Base $100k (dashed)",
        DIM,
    );

    // -------------------------------------------------------------------------
    // RIGHT COLUMN TOP: MEDIA CENTER (ILLUSTRATED SCENE + NEWS CARDS)
    // -------------------------------------------------------------------------
    let news_h = 362.0;
    let media_accent = match ui.news_source {
        Source::Newspaper => YELLOW,
        Source::Tv => RED,
        Source::Wire => CYAN,
    };
    b.panel_card(right_x, body_top, right_w, news_h, "FINANCIAL MEDIA CENTER  [N/V cycle]", media_accent);

    // Media Outlet Tabs
    let tabs = [
        (Source::Newspaper, "BULL & BEAR PAPER", YELLOW),
        (Source::Tv, "MONEY-TV 24", RED),
        (Source::Wire, "STREET WIRE", CYAN),
    ];
    let mut tx = right_x + 8.0;
    let tab_y = body_top + 25.0;
    let tab_w = (right_w - 24.0) / 3.0;
    for (src, lbl, tcol) in tabs {
        let active = ui.news_source == src;
        if active {
            b.rect_grad_v(tx, tab_y, tab_w, 16.0, [tcol[0] * 0.42, tcol[1] * 0.42, tcol[2] * 0.42, 1.0], [0.08, 0.12, 0.20, 1.0]);
            b.outline(tx, tab_y, tab_w, 16.0, tcol);
            b.text_shadow(tx + 8.0, tab_y + 4.0, 0.90, lbl, TXT);
        } else {
            b.rect(tx, tab_y, tab_w, 16.0, [0.05, 0.07, 0.12, 0.9]);
            b.outline(tx, tab_y, tab_w, 16.0, EDGE);
            b.text(tx + 8.0, tab_y + 4.0, 0.90, lbl, DIM);
        }
        tx += tab_w + 4.0;
    }

    // Illustrated Media Scene (TV Broadcast Studio / Newspaper Masthead / Wire Radar)
    let scene_y = body_top + 44.0;
    let scene_h = 62.0;
    art::draw_media_scene(
        &mut b,
        right_x + 8.0,
        scene_y,
        right_w - 16.0,
        scene_h,
        ui.news_source,
        game.market.regime,
        ui.frame,
    );

    // News Story Cards
    let mut ny = scene_y + scene_h + 6.0;
    let items = game.news_by(ui.news_source);
    let card_h = 39.0;
    for n in items.iter().take(6) {
        let sym = n.symbol.map(|i| game.market.defs[i].symbol).unwrap_or("MKT");
        let (arrow, scol, bg_tint) = if n.sentiment > 0.20 {
            ("▲", GREEN, [0.05, 0.16, 0.12, 0.85])
        } else if n.sentiment < -0.20 {
            ("▼", RED, [0.18, 0.06, 0.08, 0.85])
        } else {
            ("•", CYAN, [0.06, 0.10, 0.18, 0.85])
        };

        b.rect_grad_h(right_x + 8.0, ny, right_w - 16.0, card_h - 3.0, bg_tint, [0.04, 0.06, 0.11, 0.90]);
        b.rect(right_x + 8.0, ny, 3.5, card_h - 3.0, scol);
        b.outline(right_x + 8.0, ny, right_w - 16.0, card_h - 3.0, [0.16, 0.22, 0.34, 0.85]);

        // Symbol badge pill
        b.rect(right_x + 15.0, ny + 4.0, 44.0, 12.0, [scol[0] * 0.28, scol[1] * 0.28, scol[2] * 0.28, 1.0]);
        b.outline(right_x + 15.0, ny + 4.0, 44.0, 12.0, scol);
        b.text(right_x + 18.0, ny + 6.0, 0.88, &format!("{}{:<4}", arrow, sym), scol);

        b.text_shadow(right_x + 64.0, ny + 5.0, 0.96, &trunc(&n.headline, 44), TXT);
        b.text(right_x + 15.0, ny + 20.0, 0.88, &trunc(&n.body, 56), DIM);
        ny += card_h;
    }

    // -------------------------------------------------------------------------
    // RIGHT COLUMN BOTTOM: STREET TIPSTERS WITH CHARACTER PORTRAITS
    // -------------------------------------------------------------------------
    let tips_top = body_top + news_h + 6.0;
    let tips_h = body_bot - tips_top;
    b.panel_card(
        right_x,
        tips_top,
        right_w,
        tips_h,
        "STREET CONTACTS & TIPS  [Tab/Space/X]",
        PURPLE,
    );

    let mut ty = tips_top + 26.0;
    let tcard_h = ((tips_h - 30.0) / 4.0).clamp(36.0, 44.0);
    for (i, t) in game.tips.iter().enumerate().take(4) {
        let p = &game.people[t.person];
        let is_sel = i == ui.tip_sel;
        let sym = game.market.defs[t.symbol_idx].symbol;
        let (act_lbl, act_col) = if t.up {
            (format!("BUY {} ▲", sym), GREEN)
        } else {
            (format!("SELL {} ▼", sym), RED)
        };

        let c_bg = if is_sel {
            [0.16, 0.14, 0.32, 0.95]
        } else {
            [0.06, 0.08, 0.14, 0.90]
        };
        b.rect_grad_h(right_x + 8.0, ty, right_w - 16.0, tcard_h - 3.0, c_bg, [0.04, 0.05, 0.10, 0.92]);
        let border = if is_sel {
            ACCENT
        } else if p.trusted {
            YELLOW
        } else {
            [0.18, 0.24, 0.36, 0.9]
        };
        b.outline(right_x + 8.0, ty, right_w - 16.0, tcard_h - 3.0, border);

        // Character Portrait Avatar (Bartender, Trader, Influencer, Cabbie, Analyst, Roommate, Gym, Mod)
        let psize = (tcard_h - 9.0).clamp(26.0, 34.0);
        art::draw_portrait(&mut b, right_x + 12.0, ty + 3.0, psize, p.archetype, p.trusted, is_sel);

        let tx0 = right_x + 16.0 + psize;
        let trust_str = if p.trusted { " ★" } else { "" };
        b.text_shadow(
            tx0,
            ty + 4.0,
            0.96,
            &format!("{}{} ({})", p.name, trust_str, trunc(p.archetype, 16)),
            if is_sel { YELLOW } else { TXT },
        );

        // Target action pill on top right of tip card
        let pill_w = 78.0;
        let pill_x = right_x + right_w - pill_w - 14.0;
        b.rect(pill_x, ty + 3.0, pill_w, 13.0, [act_col[0] * 0.25, act_col[1] * 0.25, act_col[2] * 0.25, 1.0]);
        b.outline(pill_x, ty + 3.0, pill_w, 13.0, act_col);
        b.text(pill_x + 5.0, ty + 5.0, 0.88, &act_lbl, act_col);

        // Speech quote line
        b.text(tx0, ty + 19.0, 0.88, &format!("\"{}\"", trunc(&t.text, 46)), DIM);
        ty += tcard_h;
    }

    // =========================================================================
    // 3. BOTTOM TRADING CONTROL DECK + STATUS DISPLAY
    // =========================================================================
    let msg_y = h - ticker_h - deck_h;
    b.rect_grad_v(0.0, msg_y, w, deck_h, [0.07, 0.10, 0.17, 1.0], [0.04, 0.05, 0.09, 1.0]);
    b.rect_grad_h(0.0, msg_y, w, 1.5, EDGE_HI, ACCENT);

    // Recessed LED message display bar
    b.rect(10.0, msg_y + 5.0, w - 260.0, 20.0, [0.02, 0.03, 0.06, 1.0]);
    b.outline(10.0, msg_y + 5.0, w - 260.0, 20.0, [0.22, 0.34, 0.52, 1.0]);
    b.circle(20.0, msg_y + 15.0, 3.5, YELLOW);
    b.text_shadow(30.0, msg_y + 11.0, 1.0, &trunc(&ui.msg, 118), YELLOW);

    // Game Status Badge on right of control deck
    let (status_txt, scol) = match game.status {
        Status::Playing => ("● MARKET OPEN — PLAYING", GREEN),
        Status::Won => ("★ HIRED! YOU WIN $1M+ ★", GREEN),
        Status::Bankrupt => ("▼ BANKRUPT — PENNILESS", RED),
        Status::Failed => ("▼ TIME UP — UNDER $1M", RED),
    };
    let sbox_x = w - 242.0;
    b.rect_grad_h(
        sbox_x,
        msg_y + 5.0,
        232.0,
        20.0,
        [scol[0] * 0.28, scol[1] * 0.28, scol[2] * 0.28, 1.0],
        [0.05, 0.08, 0.14, 1.0],
    );
    b.outline(sbox_x, msg_y + 5.0, 232.0, 20.0, scol);
    b.text_shadow(sbox_x + 8.0, msg_y + 11.0, 0.98, status_txt, scol);

    // 3D Keycap Control Buttons Row
    let keys: &[(&str, &str, [f32; 4])] = &[
        ("↑↓/1-0", "Select", ACCENT),
        ("B", "Buy $1k", GREEN),
        ("⇧B", "Buy Max", GREEN),
        ("S", "Sell 10%", RED),
        ("⇧S", "Sell All", RED),
        ("C", "Clear Q", ORANGE),
        ("E", "END WEEK", YELLOW),
        ("Tab", "Next Tip", PURPLE),
        ("Space", "Follow Tip", PURPLE),
        ("X", "Trust", YELLOW),
        ("N/V", "Media", CYAN),
        ("F", "FSR", ACCENT),
        ("L", "LLM", ACCENT),
        ("Q", "Quit", DIM),
    ];
    let mut kx = 10.0;
    let ky = msg_y + 30.0;
    for &(k, desc, kcol) in keys {
        let kw = (k.chars().count() as f32 * 7.5 + 10.0).max(20.0);
        let dw = desc.chars().count() as f32 * 7.2 + 8.0;
        let tot_w = kw + dw + 4.0;
        if kx + tot_w > w - 8.0 {
            break;
        }
        // Keycap 3D button
        b.rect(kx, ky + 1.0, kw, 18.0, [0.02, 0.03, 0.06, 0.9]);
        b.rect_grad_v(
            kx,
            ky,
            kw,
            17.0,
            [kcol[0] * 0.45, kcol[1] * 0.45, kcol[2] * 0.45, 1.0],
            [0.08, 0.11, 0.18, 1.0],
        );
        b.outline(kx, ky, kw, 17.0, kcol);
        b.text_shadow(kx + 4.0, ky + 5.0, 0.90, k, TXT);
        b.text(kx + kw + 4.0, ky + 5.0, 0.88, desc, DIM);
        kx += tot_w + 6.0;
    }

    // =========================================================================
    // 4. SCROLLING ELECTRONIC STOCK TICKER TAPE (BOTTOM)
    // =========================================================================
    let ty = h - ticker_h;
    b.rect(0.0, ty, w, ticker_h, TICKER_BG);
    b.rect_grad_h(0.0, ty, w, 1.5, ACCENT, PURPLE);

    // Scrolling ticker segments
    let mut segs: Vec<(String, String, [f32; 4])> = Vec::new();
    for (i, d) in game.market.defs.iter().enumerate() {
        let st = &game.market.states[i];
        let wk = if st.history.len() >= 2 {
            st.history[st.history.len() - 1] / st.history[st.history.len() - 2] - 1.0
        } else {
            0.0
        };
        let col = if wk >= 0.0 { GREEN } else { RED };
        let arrow = if wk >= 0.0 { "▲" } else { "▼" };
        segs.push((
            d.symbol.to_string(),
            format!("{} {}{}   ", fmt_price(prices[i]), arrow, pct(wk)),
            col,
        ));
    }
    let item_w = 186.0;
    let full_w = (segs.len() as f32 * item_w).max(1.0);
    let mut offset = (ui.frame as f32 * 1.6) % full_w;
    if offset < 0.0 {
        offset += full_w;
    }

    for pass in 0..2 {
        let mut x = 86.0 - offset + pass as f32 * full_w;
        for (sym, rest, col) in &segs {
            if x >= 80.0 && x < w {
                b.rect(x, ty + 5.0, 38.0, 18.0, [col[0] * 0.22, col[1] * 0.22, col[2] * 0.22, 0.95]);
                b.outline(x, ty + 5.0, 38.0, 18.0, *col);
                b.text_shadow(x + 4.0, ty + 10.0, 0.95, sym, TXT);
                b.text_shadow(x + 43.0, ty + 10.0, 0.95, rest, *col);
            }
            x += item_w;
        }
    }

    // Fixed left "LIVE TAPE" badge over the scrolling ribbon
    b.rect_grad_h(0.0, ty + 2.0, 78.0, ticker_h - 2.0, [0.78, 0.12, 0.16, 1.0], [0.45, 0.06, 0.10, 1.0]);
    b.outline(0.0, ty + 2.0, 78.0, ticker_h - 2.0, RED);
    let dot_col = if (ui.frame / 12) % 2 == 0 { [1.0, 1.0, 1.0, 1.0] } else { YELLOW };
    b.circle(10.0, ty + 15.0, 3.5, dot_col);
    b.text_shadow(18.0, ty + 11.0, 0.95, "LIVE TAPE", TXT);

    b
}
