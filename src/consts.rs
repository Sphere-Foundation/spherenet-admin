use solana_sdk::pubkey::Pubkey;
use std::sync::LazyLock;

pub const RPC_URL: &str = "https://api.testnet.sphere.net";

pub static SYSTEM_PROGRAM: LazyLock<Pubkey> = LazyLock::new(|| Pubkey::default());
