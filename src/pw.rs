use crate::consts::RPC_URL;
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_program_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ProgramWhitelistAccount, load, whitelist_entry::ProgramWhitelistEntry},
};

pub fn list() -> Result<()> {
    // Connect to testnet
    let rpc_client = RpcClient::new(RPC_URL);

    // Get the program whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ProgramWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize program whitelist account: {:?}", e))?;

    // Print authority info
    println!("\nProgram Whitelist");
    println!("  Authority:         {}", Pubkey::from(whitelist.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(whitelist.pending_authority)
    );

    println!("\nWhitelisted Programs:");

    // Use getProgramAccounts to find all program whitelist entries
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let accounts = rpc_client.get_program_accounts(&program_id)?;

    let mut found_count = 0;
    for (pubkey, account) in accounts {
        // Skip the main whitelist account itself
        if pubkey == whitelist_pubkey {
            continue;
        }

        // Try to deserialize as ProgramWhitelistEntry
        if let Ok(entry_data) = load::<ProgramWhitelistEntry>(&account.data) {
            println!(
                "  Program Address:  {}",
                Pubkey::from(entry_data.entry_address)
            );
            println!();

            found_count += 1;
        }
    }

    if found_count == 0 {
        println!("  No program entries found");
    }

    Ok(())
}
