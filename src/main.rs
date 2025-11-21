mod cli;
mod loader;
mod pw;
mod utils;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
