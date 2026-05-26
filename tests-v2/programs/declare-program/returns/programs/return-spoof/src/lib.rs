use anchor_lang_v2::prelude::*;

declare_id!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

#[program]
pub mod return_spoof {
    use super::*;

    #[discrim = 9]
    pub fn spoof(_ctx: &mut Context<Spoof>, value: u64) -> Result<u64> {
        Ok(value.saturating_add(1_000))
    }
}

#[derive(Accounts)]
pub struct Spoof {
    pub data: UncheckedAccount,
}
