mod cli;
mod loader;
mod mp;
mod pw;
mod stake;
mod utils;
mod vote;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
