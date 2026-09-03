use anchor_lang::prelude::*;

pub const FOREIGN_BORSH_OWNER: &str = "Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u";

declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[derive(Clone, Default)]
#[account(borsh)]
pub struct ForeignBorshCounter {
    pub value: u64,
}
