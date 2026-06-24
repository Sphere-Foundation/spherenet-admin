mod cli;
mod loader;
mod mp;
mod pw;
mod utils;
mod vote;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
