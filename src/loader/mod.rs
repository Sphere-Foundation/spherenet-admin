pub mod deploy;
pub mod upgrade;

use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spherenet_whitelisted_loader_v3_interface::instruction::write;

/// Conservative chunk size for writing program data to avoid transaction size limits
const MAX_WRITE_SIZE: usize = 900;

/// Writes program data to a buffer account in chunks.
///
/// Program data is split into MAX_WRITE_SIZE chunks (900 bytes) to avoid hitting
/// transaction size limits. Each chunk is written via a separate transaction signed
/// by both the payer (who pays transaction fees) and the authority (buffer owner).
///
/// # Arguments
/// * `rpc_client` - RPC client for submitting transactions
/// * `payer` - Keypair that pays for transaction fees
/// * `authority_keypair` - Buffer authority keypair (must sign each write)
/// * `buffer` - Address of the buffer account to write to
/// * `authority` - Authority pubkey (must match buffer's authority)
/// * `program_data` - Complete program bytecode to write
pub fn write_buffer(
    rpc_client: &RpcClient,
    payer: &Keypair,
    authority_keypair: &Keypair,
    buffer: &Pubkey,
    authority: &Pubkey,
    program_data: &[u8],
) -> Result<()> {
    let chunks: Vec<_> = program_data.chunks(MAX_WRITE_SIZE).collect();
    let total_chunks = chunks.len();

    for (i, chunk) in chunks.into_iter().enumerate() {
        let offset = i * MAX_WRITE_SIZE;
        print!(
            "  Writing chunk {}/{} ({} bytes)...",
            i + 1,
            total_chunks,
            chunk.len()
        );

        let write_ix = write(buffer, authority, offset as u32, chunk.to_vec());

        let mut transaction = Transaction::new_with_payer(&[write_ix], Some(&payer.pubkey()));
        transaction.sign(
            &[payer, authority_keypair],
            rpc_client.get_latest_blockhash()?,
        );
        rpc_client.send_and_confirm_transaction(&transaction)?;

        println!(" ✓");
    }

    println!("  ✓ Program data written successfully");
    Ok(())
}
