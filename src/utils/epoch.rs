//! Show the current epoch.

use crate::cli::output::{emit, field, OutputMode, Render};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;

#[derive(serde::Serialize)]
struct EpochView {
    epoch: u64,
    slot_index: u64,
    slots_in_epoch: u64,
    absolute_slot: u64,
}

impl Render for EpochView {
    fn to_text(&self) -> String {
        let pct = if self.slots_in_epoch > 0 {
            self.slot_index as f64 / self.slots_in_epoch as f64 * 100.0
        } else {
            0.0
        };
        let mut out = field("Epoch", self.epoch);
        out.push_str(&field(
            "Slot Index",
            format!("{} / {} ({:.2}%)", self.slot_index, self.slots_in_epoch, pct),
        ));
        out.push_str(&field("Absolute Slot", self.absolute_slot));
        out
    }
}

pub fn epoch(rpc_url: &str, mode: OutputMode) -> eyre::Result<()> {
    let rpc_client =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let info = rpc_client.get_epoch_info()?;
    let view = EpochView {
        epoch: info.epoch,
        slot_index: info.slot_index,
        slots_in_epoch: info.slots_in_epoch,
        absolute_slot: info.absolute_slot,
    };
    emit(&view, mode)
}
