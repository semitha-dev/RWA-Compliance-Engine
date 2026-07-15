use anchor_lang::prelude::*;

#[event]
pub struct TransferChecked {
    pub mint: Pubkey,
    pub source_owner: Pubkey,
    pub destination_owner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ExtraAccountMetaListInitialized {
    pub mint: Pubkey,
}
