//! Vulkan UI (windowed via winit, or headless via VK_EXT_headless_surface).

pub mod app;
pub mod draw;
pub mod fsr;
pub mod png;
pub mod renderer;
pub mod shaders;

use crate::sim::game::Game;

pub struct VkOptions {
    pub width: u32,
    pub height: u32,
    pub fsr: bool,
    pub fsr_scale: f32,
    pub headless: bool,
    pub frames: u32,
    pub screenshot: Option<String>,
}

pub fn run_window(game: &mut Game, opts: &VkOptions) -> i32 {
    app::run_window(game, opts)
}

pub fn run_headless_gpu(game: &mut Game, opts: &VkOptions) -> i32 {
    app::run_headless_gpu(game, opts)
}
