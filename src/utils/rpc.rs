//! RPC account reads that keep "missing" and "RPC failed" apart.
//!
//! `RpcClient::get_account` turns a missing account into an error, so a caller
//! that treats any error as "absent" also reads an RPC outage as "absent" — a
//! misleading "not found" at best, and a fail-open guard at worst. Read through
//! [`get_account_opt`] instead.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};

/// Fetch an account at the client's commitment.
///
/// `Ok(None)` only if the account genuinely doesn't exist; `Err` on an RPC
/// failure, so callers never mistake an outage for a missing account.
pub fn get_account_opt(rpc: &RpcClient, pubkey: &Pubkey) -> eyre::Result<Option<Account>> {
    rpc.get_account_with_commitment(pubkey, rpc.commitment())
        .map(|response| response.value)
        .map_err(|e| eyre::eyre!("RPC error reading account {pubkey}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An unreachable RPC must surface as an error — never as `Ok(None)`, which
    /// callers read as "the account doesn't exist".
    #[test]
    fn rpc_failure_is_an_error_not_a_missing_account() {
        // Port 1 on localhost: nothing listens there, so the request fails fast.
        let rpc = RpcClient::new("http://127.0.0.1:1".to_string());
        let err = get_account_opt(&rpc, &Pubkey::new_unique()).unwrap_err();
        assert!(err.to_string().contains("RPC error"), "got: {err}");
    }
}
