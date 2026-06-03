//! Only tests whether `anchor-spl` builds with `metadata` feature enabled.

use anchor_lang::prelude::*;

declare_id!("Metadata11111111111111111111111111111111111");

#[program]
pub mod metadata {}
