use anchor_lang::prelude::*;

/// Same name as `alias_narrow::SameNameAlias`, different definition.
pub type SameNameAlias = [u8; 4];

#[account]
pub struct WideAliasAccount {
    pub value: SameNameAlias,
}
