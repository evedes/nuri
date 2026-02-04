// TODO: remove once pipeline modules are wired into main
#![allow(dead_code)]

mod cli;
mod pipeline;
mod theme;
mod tui;

use anyhow::Result;
use clap::Parser;

use cli::Args;

fn main() -> Result<()> {
    let _args = Args::parse();

    // TODO(ticket 9): wire up the full pipeline
    // 1. load_and_prepare()
    // 2. extract_colors()
    // 3. detect_mode()
    // 4. assign_slots()
    // 5. enforce_contrast()
    // 6. GhosttyTheme::from_palette()
    // 7. output (--install / -o / stdout)

    eprintln!("ghostty-themer: pipeline not yet implemented");
    Ok(())
}
