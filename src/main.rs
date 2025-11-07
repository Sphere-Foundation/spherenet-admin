mod airdrop;
mod cli;
mod loader;
mod pw;
mod squads;
mod vw;

fn main() -> eyre::Result<()> {
    cli::run()
}
