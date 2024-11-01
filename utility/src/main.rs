#!/usr/bin/env -S cargo run --quiet --package utility --

use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    match args.get(1).map(|s| s.as_str()) {
        Some("--run-dev") => {
            let status = Command::new("cargo")
                .env("WGPU_BACKEND", "vulkan")
                .env("BEVY_RENDER_BACKEND", "vulkan")
                .env("RUST_BACKTRACE", "1")
                .args(["run", "--release"])
                .current_dir("..")
                .status()
                .expect("Failed to execute command");

            std::process::exit(status.code().unwrap_or(1));
        }
        _ => {
            println!("Available commands:");
            println!("  --run-dev    Run the game in development mode with Vulkan backend");
        }
    }
}
