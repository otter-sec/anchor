use anchor_lang::prelude::*;

/// Same name as `alias_wide::SameNameAlias`, different definition.
pub type SameNameAlias = u8;

#[account]
pub struct NarrowAliasAccount {
    pub value: SameNameAlias,
}
