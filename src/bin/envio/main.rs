mod clap_app;
mod cli;
mod commands;
mod utils;
mod version;

use clap::Parser;
use colored::Colorize;

use clap_app::ClapApp;

#[cfg(target_family = "unix")]
use utils::initalize_config;

fn main() {
    let args = ClapApp::parse();

    #[cfg(target_family = "unix")]
    if let Err(e) = initalize_config() {
        println!("{}: {}", "Error".red(), e);
    }

    if let Err(e) = args.command.run() {
        println!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}
