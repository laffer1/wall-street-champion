//! Vulkan renderer entry points (full implementation follows).
use crate::sim::game::Game;
use super::VkOptions;

pub fn run_window(_game: &mut Game, _opts: &VkOptions) -> i32 {
    eprintln!("Vulkan UI not built yet in this commit");
    1
}

pub fn run_headless_gpu(_game: &mut Game, _opts: &VkOptions) -> i32 {
    eprintln!("Vulkan UI not built yet in this commit");
    1
}
