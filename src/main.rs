mod cli;
mod loader;
mod pw;
mod squads;
mod utils;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
