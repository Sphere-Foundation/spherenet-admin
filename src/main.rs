use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spherenet_validator_whitelist_interface::{
    account_solana, program_solana,
    state::{account::ValidatorWhitelistAccount, load, whitelist_entry::ValidatorWhitelistEntry},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to your local Solana node
    let rpc_url = "https://api.testnet.sphere.net";
    let rpc_client = RpcClient::new(rpc_url);

    // Replace this with your actual account pubkey
    let account_pubkey = Pubkey::from(account_solana::id().to_bytes());

    println!("\nFetching account data for: {}", account_pubkey);

    // Get the account data
    let account = rpc_client.get_account(&account_pubkey)?;

    println!("Account data size: {} bytes", account.data.len());

    // Deserialize the data
    let whitelist = unsafe { load::<ValidatorWhitelistAccount>(&account.data) }.unwrap();

    // Print in a human-readable format
    println!("\nValidator Whitelist:");
    println!("  Authority: {}", Pubkey::from(whitelist.authority));
    println!(
        "  Pending Authority: {}",
        Pubkey::from(whitelist.pending_authority)
    );
    println!(
        "  Validator Amount: {}",
        u32::from_le_bytes(whitelist.validator_amount)
    );

    let vote_account_pubkey = Pubkey::from_str("EoJCeP12QGb1PcG4AMrT5bQYJsRb7iwDvnKaaLgdtXvs")?;
    let entry_pubkey = Pubkey::find_program_address(
        &[vote_account_pubkey.to_bytes().as_ref()],
        &Pubkey::from(program_solana::id().to_bytes()),
    )
    .0;
    let entry_account = rpc_client.get_account(&entry_pubkey)?;
    let entry_data = unsafe { load::<ValidatorWhitelistEntry>(&entry_account.data) }.unwrap();

    println!("Entry data size: {} bytes", entry_account.data.len());
    println!("\nEntry data:");
    println!("  Vote Account: {}", Pubkey::from(entry_data.pubkey));
    println!(
        "  Start Epoch: {}",
        u64::from_le_bytes(entry_data.start_epoch)
    );
    println!("  End Epoch: {}", u64::from_le_bytes(entry_data.end_epoch));

    Ok(())
}
