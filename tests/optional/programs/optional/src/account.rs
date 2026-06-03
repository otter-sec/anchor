use anchor_lang::prelude::*;

#[account]
pub struct DataPda {
    pub data_account: Pubkey,
}

impl DataPda {
    pub const LEN: usize = 8 + 32;
    pub const PREFIX: &'static str = "data_pda";
}

#[account]
pub struct DataAccount {
    pub data: u64,
}

impl DataAccount {
    pub const LEN: usize = 8 + 8;
}
