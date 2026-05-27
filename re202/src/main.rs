use anyhow::Result;
use clap::Parser;

mod cli;
mod midi;
mod yaml_io;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = cli::Cli::parse();
    cli::run(args)
}
