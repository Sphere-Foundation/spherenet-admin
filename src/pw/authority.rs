use crate::consts::RPC_URL;
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_program_whitelist_interface::{
    account_solana,
    state::{account::ProgramWhitelistAccount, load},
};

pub fn auth() -> Result<()> {
    // Connect to testnet
    let rpc_client = RpcClient::new(RPC_URL);

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ProgramWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize program whitelist account: {:?}", e))?;

    // Print authority info
    println!("\nProgram Whitelist");
    println!("  Whitelist Account: {}", whitelist_pubkey);
    println!("  Authority:         {}", Pubkey::from(whitelist.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(whitelist.pending_authority)
    );
    println!();

    Ok(())
}
