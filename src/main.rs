//! Wall Street Champion — entry point.
//!
//! Modes:
//!   (default)              Vulkan window game
//!   --headless [--bot]     terminal game (--bot auto-plays 26 weeks)
//!   --gpu-headless         render on a VK_EXT_headless_surface (no X needed)
//!   --frames N --screenshot out.png   with --gpu-headless: dump a PNG
//!   --seed N               deterministic market
//!   --llm                  use a local Ollama model for news bodies

use wall_street_champion::sim::game::Game;
use wall_street_champion::ui::vk::VkOptions;

fn main() {
    std::process::exit(real_main());
}

fn real_main() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut headless = false;
    let mut bot = false;
    let mut gpu_headless = false;
    let mut llm_flag = false;
    let mut seed: Option<u64> = None;
    let mut frames = 30u32;
    let mut screenshot: Option<String> = None;
    let mut width = 1280u32;
    let mut height = 720u32;
    let mut fsr = true;
    let mut fsr_scale = 1.0f32;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--headless" => headless = true,
            "--bot" => bot = true,
            "--gpu-headless" => gpu_headless = true,
            "--llm" => llm_flag = true,
            "--no-fsr" => fsr = false,
            "--seed" => {
                i += 1;
                seed = args.get(i).and_then(|s| s.parse().ok());
            }
            "--frames" => {
                i += 1;
                frames = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(frames);
            }
            "--screenshot" => {
                i += 1;
                screenshot = args.get(i).cloned();
            }
            "--width" => {
                i += 1;
                width = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(width);
            }
            "--height" => {
                i += 1;
                height = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(height);
            }
            "--fsr-scale" => {
                i += 1;
                fsr_scale = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(fsr_scale);
            }
            "--help" | "-h" => {
                println!("wall-street-champion [--headless [--bot]] [--gpu-headless --frames N --screenshot out.png] [--seed N] [--llm] [--no-fsr] [--fsr-scale 0.66] [--width W --height H]");
                return 0;
            }
            other => {
                eprintln!("unknown arg: {}", other);
                return 2;
            }
        }
        i += 1;
    }

    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(42)
    });

    let llm = if llm_flag || std::env::var("WSC_LLM").as_deref() == Ok("1") {
        match wall_street_champion::llm::Llm::probe() {
            Some(l) => {
                eprintln!("local LLM connected: model {} on {}", l.model, l.addr);
                Some(l)
            }
            None => {
                eprintln!("local LLM not reachable; using procedural news");
                None
            }
        }
    } else {
        None
    };

    if gpu_headless {
        let mut game = Game::new(seed, llm);
        let opts = VkOptions { width, height, fsr, fsr_scale, headless: true, frames, screenshot };
        return wall_street_champion::ui::vk::run_headless_gpu(&mut game, &opts);
    }
    if headless {
        return wall_street_champion::ui::cli::run(seed, llm, bot);
    }
    let mut game = Game::new(seed, llm);
    let opts = VkOptions { width, height, fsr, fsr_scale, headless: false, frames, screenshot: None };
    wall_street_champion::ui::vk::run_window(&mut game, &opts)
}
