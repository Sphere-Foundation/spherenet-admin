use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_validator_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ValidatorWhitelistAccount, load, whitelist_entry::ValidatorWhitelistEntry},
};

pub fn list() -> Result<()> {
    // Connect to testnet
    let rpc_url = "https://api.testnet.sphere.net";
    let rpc_client = RpcClient::new(rpc_url);

    // Get the validator whitelist account
    let whitelist_pubkey = Pubkey::from(account_solana::id().to_bytes());
    let account = rpc_client.get_account(&whitelist_pubkey)?;
    let whitelist = load::<ValidatorWhitelistAccount>(&account.data)
        .map_err(|e| eyre::eyre!("Failed to deserialize whitelist account: {:?}", e))?;

    // Print authority info
    println!("\nValidator Whitelist");
    println!("  Authority:         {}", Pubkey::from(whitelist.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(whitelist.pending_authority)
    );
    println!(
        "  Validator Count:   {}",
        u32::from_le_bytes(whitelist.validator_amount)
    );

    // Get validator count
    let validator_count = u32::from_le_bytes(whitelist.validator_amount);

    if validator_count == 0 {
        println!("\nNo validators whitelisted.");
        return Ok(());
    }

    println!("\nWhitelisted Validators:");

    // Use getProgramAccounts to find all validator whitelist entries
    let program_id = Pubkey::from(program_solana::id().to_bytes());
    let accounts = rpc_client.get_program_accounts(&program_id)?;

    let mut found_count = 0;
    for (pubkey, account) in accounts {
        // Skip the main whitelist account itself
        if pubkey == whitelist_pubkey {
            continue;
        }

        // Try to deserialize as ValidatorWhitelistEntry
        if let Ok(entry_data) = load::<ValidatorWhitelistEntry>(&account.data) {
            let start_epoch = u64::from_le_bytes(entry_data.start_epoch);
            let end_epoch = u64::from_le_bytes(entry_data.end_epoch);

            println!("  Vote Account:  {}", Pubkey::from(entry_data.pubkey));
            println!("  Start Epoch:   {}", start_epoch);
            println!(
                "  End Epoch:     {}",
                if end_epoch == u64::MAX {
                    "∞".to_string()
                } else {
                    end_epoch.to_string()
                }
            );
            println!();

            found_count += 1;
        }
    }

    if found_count == 0 {
        println!("  No validator entries found");
    }

    Ok(())
}
