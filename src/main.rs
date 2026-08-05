mod cli;
mod kms;
mod loader;
mod mp;
mod pw;
mod server;
mod squads;
mod stake;
mod utils;
mod vote;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
