mod authority;
mod cli;
mod loader;
mod mp;
mod pw;
mod server;
mod stake;
mod utils;
mod vote;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
