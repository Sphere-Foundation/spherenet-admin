//! Show the current epoch.
//!
//! Output is just the epoch number (matching the client CLI), so scripts can
//! use it directly — e.g. `vw add --start-epoch "$(spherenet-admin epoch)"`.

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;

pub fn epoch(rpc_url: &str) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let epoch_info = rpc_client.get_epoch_info()?;
    println!("{}", epoch_info.epoch);

    Ok(())
}
