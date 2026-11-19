//! Entry points for the Vulkan UI: windowed (winit) and headless
//! (VK_EXT_headless_surface, used for --gpu-headless screenshots).

use ash::{ext, khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;

use crate::sim::game::{Game, Status, TOTAL_WEEKS};
use crate::sim::news::Source;
use crate::sim::portfolio::{fmt_money, Order, Side};
use super::draw::{self, UiState};
use super::png;
use super::renderer::Core;
use super::VkOptions;

// ---------------------------------------------------------------------------
// headless GPU mode
// ---------------------------------------------------------------------------

pub fn run_headless_gpu(game: &mut Game, opts: &VkOptions) -> i32 {
    match run_headless_inner(game, opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("vulkan headless error: {}", e);
            1
        }
    }
}

fn run_headless_inner(game: &mut Game, opts: &VkOptions) -> Result<i32, String> {
    let exts = [ext::headless_surface::NAME.as_ptr(), khr::surface::NAME.as_ptr()];
    let (entry, instance) = Core::base_instance(&exts)?;
    let headless = ext::headless_surface::Instance::new(&entry, &instance);
    let surface = unsafe { headless.create_headless_surface(&vk::HeadlessSurfaceCreateInfoEXT::default(), None) }
        .map_err(|e| format!("headless surface: {:?}", e))?;
    let scale = if opts.fsr { opts.fsr_scale } else { 1.0 };
    let mut core = Core::new_common(entry, instance, surface, opts.width, opts.height, scale)?;
    eprintln!(
        "headless Vulkan: {} {}x{} (render {}x{}, FSR1.x={}) on {}",
        format!("{:?}", core.sc_format),
        core.extent.width,
        core.extent.height,
        core.lw,
        core.lh,
        opts.fsr,
        core.device_name()
    );
    let mut ui = UiState::new();
    ui.fsr = opts.fsr;
    ui.fsr_scale = opts.fsr_scale;
    ui.llm_on = game.llm.is_some();
    let shot_frame = opts.frames.saturating_sub(1);
    for frame in 0..opts.frames {
        if frame == 1 && game.status == Status::Playing {
            let prices = game.prices();
            for &(sym_idx, dollars) in &[(0usize, 28_000.0), (12, 18_000.0), (17, 14_000.0)] {
                if sym_idx < prices.len() {
                    game.portfolio.queue(Order {
                        symbol_idx: sym_idx,
                        side: Side::Buy,
                        shares: dollars / prices[sym_idx],
                    });
                }
            }
            game.toggle_trust(0);
        }
        if frame == 16 && game.status == Status::Playing {
            let prices = game.prices();
            game.portfolio.queue(Order {
                symbol_idx: 1,
                side: Side::Buy,
                shares: 5_000.0 / prices[1],
            });
        }
        ui.news_source = match (frame / 10) % 3 {
            0 => Source::Newspaper,
            1 => Source::Tv,
            _ => Source::Wire,
        };
        if frame % 8 == 7 && game.status == Status::Playing {
            match game.end_week() {
                Ok(r) => {
                    ui.msg = format!(
                        "Week {} started. {} fills, net worth {}",
                        r.week.min(TOTAL_WEEKS),
                        r.fills.len(),
                        fmt_money(r.net_worth_cents)
                    )
                }
                Err(e) => ui.msg = format!("order rejected: {}", e),
            }
        }
        ui.frame = frame as u64;
        let batch = draw::build(game, &ui, core.extent.width as f32, core.extent.height as f32);
        let want_shot = frame == shot_frame;
        core.render(&batch, opts.fsr, want_shot)?;
    }
    if let Some(path) = &opts.screenshot {
        core.wait_idle()?;
        let rgba = core.take_shot();
        // probe: average color of top strip, bottom strip, left strip, right strip
        let (w, h) = (core.extent.width as usize, core.extent.height as usize);
        let avg = |x0: usize, y0: usize, x1: usize, y1: usize| -> (u32, u32, u32) {
            let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) * 4;
                    r += rgba[i] as u64;
                    g += rgba[i + 1] as u64;
                    b += rgba[i + 2] as u64;
                    n += 1;
                }
            }
            ((r / n) as u32, (g / n) as u32, (b / n) as u32)
        };
        eprintln!("probe top strip: {:?}  bottom strip: {:?}", avg(0, 0, w, 20), avg(0, h - 26, w, h));
        eprintln!("probe left strip: {:?}  right strip: {:?}", avg(0, 0, 20, h), avg(w - 20, 0, w, h));
        png::write_png(path, core.extent.width, core.extent.height, &rgba).map_err(|e| format!("png: {}", e))?;
        eprintln!("screenshot written to {}", path);
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// windowed mode
// ---------------------------------------------------------------------------

pub fn run_window(game: &mut Game, opts: &VkOptions) -> i32 {
    use winit::event_loop::EventLoop;

    let event_loop = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("no event loop (no display?): {}", e);
            eprintln!("hint: use --headless for terminal mode or --gpu-headless for offscreen rendering");
            return 1;
        }
    };
    let placeholder = Game::new(0, None);
    let mut app = App {
        opts: VkOptions {
            width: opts.width,
            height: opts.height,
            fsr: opts.fsr,
            fsr_scale: opts.fsr_scale,
            headless: false,
            frames: 0,
            screenshot: None,
        },
        game: Some(std::mem::replace(game, placeholder)),
        ui: UiState::new(),
        core: None,
        window: None,
        mods: winit::keyboard::ModifiersState::empty(),
    };
    let _ = event_loop.run_app(&mut app);
    match app.game.as_ref().map(|g| g.status) {
        Some(Status::Won) => 0,
        _ => 1,
    }
}

struct App {
    opts: VkOptions,
    game: Option<Game>,
    ui: UiState,
    core: Option<Core>,
    window: Option<Arc<winit::window::Window>>,
    mods: winit::keyboard::ModifiersState,
}

impl App {
    fn handle_key(&mut self, code: winit::keyboard::KeyCode, shift: bool) {
        let Some(game) = self.game.as_mut() else { return };
        let n = game.market.defs.len();
        let sel = self.ui.selected.min(n - 1);
        use winit::keyboard::KeyCode::*;
        match code {
            Escape | KeyQ => {
                println!(
                    "quit. final net worth: {} (status {:?})",
                    fmt_money(game.net_worth_cents()),
                    game.status
                );
                std::process::exit(match game.status {
                    Status::Won => 0,
                    _ => 1,
                });
            }
            KeyE => match game.end_week() {
                Ok(r) => {
                    self.ui.msg = format!(
                        "Week {} began. {} fills, dividends {}, expenses -{}, interest -{}, net worth {}",
                        r.week.min(TOTAL_WEEKS),
                        r.fills.len(),
                        fmt_money(r.dividends_cents),
                        fmt_money(r.expenses_cents),
                        fmt_money(r.interest_cents),
                        fmt_money(r.net_worth_cents)
                    );
                    if game.status != Status::Playing {
                        self.ui.msg = match game.status {
                            Status::Won => "YOU REACHED $1,000,000 — a trading firm HIRED you! (Q to quit)".to_string(),
                            Status::Bankrupt => "BANKRUPT — penniless. (Q to quit)".to_string(),
                            Status::Failed => "26 weeks over, under $1M — penniless. (Q to quit)".to_string(),
                            Status::Playing => self.ui.msg.clone(),
                        };
                    }
                }
                Err(e) => self.ui.msg = format!("orders rejected: {}", e),
            },
            KeyB => {
                let prices = game.prices();
                let price = prices[sel];
                let bp = game.portfolio.buying_power_cents(&prices).max(0) as f64 / 100.0;
                let amt = if shift { (bp * 0.95).max(0.0) } else { 1000.0 };
                let shares = amt / price;
                game.portfolio.queue(Order { symbol_idx: sel, side: Side::Buy, shares });
                self.ui.msg = format!("queued BUY {} {:.2} shares (~${:.0})", game.market.defs[sel].symbol, shares, amt);
            }
            KeyS => {
                let have = game.portfolio.shares_of(sel);
                if have > 0.0 {
                    let shares = if shift { have } else { have * 0.1 };
                    game.portfolio.queue(Order { symbol_idx: sel, side: Side::Sell, shares });
                    self.ui.msg = format!("queued SELL {} {:.2} shares", game.market.defs[sel].symbol, shares);
                } else {
                    self.ui.msg = format!("you hold no {}", game.market.defs[sel].symbol);
                }
            }
            KeyC => {
                game.portfolio.queued.clear();
                self.ui.msg = "cleared all queued orders".to_string();
            }
            KeyN => {
                self.ui.news_source = match self.ui.news_source {
                    Source::Newspaper => Source::Tv,
                    Source::Tv => Source::Wire,
                    Source::Wire => Source::Newspaper,
                };
            }
            KeyV => {
                self.ui.news_source = if self.ui.news_source == Source::Tv {
                    Source::Newspaper
                } else {
                    Source::Tv
                };
            }
            KeyW => {
                self.ui.news_source = Source::Wire;
            }
            ArrowUp | ArrowLeft => {
                self.ui.selected = (sel + n - 1) % n;
            }
            ArrowDown | ArrowRight => {
                self.ui.selected = (sel + 1) % n;
            }
            KeyF => {
                self.ui.fsr = !self.ui.fsr;
                if let Some(core) = self.core.as_mut() {
                    let scale = if self.ui.fsr { self.opts.fsr_scale } else { 1.0 };
                    match core.set_render_scale(scale) {
                        Ok(()) => self.ui.msg = format!("FSR 1.x {} (render scale {:.2}x)", if self.ui.fsr { "ON" } else { "OFF" }, scale),
                        Err(e) => self.ui.msg = format!("FSR toggle failed: {}", e),
                    }
                }
            }
            KeyL => {
                if game.llm.is_none() {
                    game.llm = crate::llm::Llm::probe();
                } else {
                    game.llm = None;
                }
                self.ui.llm_on = game.llm.is_some();
                self.ui.msg = if self.ui.llm_on { "local LLM news ON".to_string() } else { "local LLM news OFF".to_string() };
            }
            KeyH => self.ui.show_help = !self.ui.show_help,
            Tab => {
                if !game.tips.is_empty() {
                    self.ui.tip_sel = (self.ui.tip_sel + 1) % game.tips.len();
                }
            }
            Space => {
                if !game.tips.is_empty() {
                    match game.follow_tip(self.ui.tip_sel) {
                        Ok(m) => self.ui.msg = m,
                        Err(e) => self.ui.msg = e,
                    }
                }
            }
            KeyX => {
                if let Some(t) = game.tips.get(self.ui.tip_sel) {
                    let p = t.person;
                    game.toggle_trust(p);
                    self.ui.msg = format!("trust toggled for {}", game.people[p].name);
                }
            }
            Digit1 => self.ui.selected = 0,
            Digit2 => self.ui.selected = 1 % n,
            Digit3 => self.ui.selected = 2 % n,
            Digit4 => self.ui.selected = 3 % n,
            Digit5 => self.ui.selected = 4 % n,
            Digit6 => self.ui.selected = 5 % n,
            Digit7 => self.ui.selected = 6 % n,
            Digit8 => self.ui.selected = 7 % n,
            Digit9 => self.ui.selected = 8 % n,
            Digit0 => self.ui.selected = 9 % n,
            Minus => self.ui.selected = 10 % n,
            Equal => self.ui.selected = 11 % n,
            _ => {}
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        use winit::dpi::LogicalSize;
        use winit::window::Window;
        let attrs = Window::default_attributes()
            .with_title("Wall Street Champion")
            .with_inner_size(LogicalSize::new(self.opts.width as f64, self.opts.height as f64));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("create window: {}", e);
                event_loop.exit();
                return;
            }
        };
        let exts = match ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw()) {
            Ok(e) => e.to_vec(),
            Err(e) => {
                eprintln!("surface extensions: {:?}", e);
                event_loop.exit();
                return;
            }
        };
        let (entry, instance) = match Core::base_instance(&exts) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", e);
                event_loop.exit();
                return;
            }
        };
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
        };
        let surface = match surface {
            Ok(s) => s,
            Err(e) => {
                eprintln!("create surface: {:?}", e);
                event_loop.exit();
                return;
            }
        };
        let scale = if self.opts.fsr { self.opts.fsr_scale } else { 1.0 };
        match Core::new_common(entry, instance, surface, self.opts.width, self.opts.height, scale) {
            Ok(core) => {
                eprintln!("vulkan device: {} (FSR 1.x {})", core.device_name(), if self.opts.fsr { "on" } else { "off" });
                self.ui.fsr = self.opts.fsr;
                self.ui.fsr_scale = self.opts.fsr_scale;
                if let Some(g) = &self.game {
                    self.ui.llm_on = g.llm.is_some();
                }
                self.core = Some(core);
                self.window = Some(window);
            }
            Err(e) => {
                eprintln!("renderer init: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;
        use winit::keyboard::PhysicalKey;
        match event {
            WindowEvent::CloseRequested => {
                if let Some(g) = &self.game {
                    println!(
                        "final net worth: {} (status {:?})",
                        fmt_money(g.net_worth_cents()),
                        g.status
                    );
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(core) = self.core.as_mut() {
                    if size.width > 0 && size.height > 0 {
                        let scale = if self.ui.fsr { self.opts.fsr_scale } else { 1.0 };
                        let _ = core.resize(size.width, size.height, scale);
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == winit::event::ElementState::Pressed {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        let shift = self.mods.contains(winit::keyboard::ModifiersState::SHIFT);
                        self.handle_key(code, shift);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(core), Some(game)) = (self.core.as_mut(), self.game.as_ref()) {
                    if let Some(w) = &self.window {
                        let sz = w.inner_size();
                        if sz.width > 0 && sz.height > 0 && (sz.width != core.extent.width || sz.height != core.extent.height) {
                            let scale = if self.ui.fsr { self.opts.fsr_scale } else { 1.0 };
                            let _ = core.resize(sz.width, sz.height, scale);
                        }
                    }
                    self.ui.frame += 1;
                    let batch = draw::build(game, &self.ui, core.extent.width as f32, core.extent.height as f32);
                    match core.render(&batch, self.ui.fsr, false) {
                        Ok(()) => {}
                        Err(e) if e == "out_of_date" => {
                            if let Some(w) = &self.window {
                                let sz = w.inner_size();
                                let scale = if self.ui.fsr { self.opts.fsr_scale } else { 1.0 };
                                let _ = core.resize(sz.width, sz.height, scale);
                            }
                        }
                        Err(e) => eprintln!("render: {}", e),
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}
