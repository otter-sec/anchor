use anchor_lang::prelude::*;

declare_id!("2uA3amp95zsEHUpo8qnLMhcFAUsiKVEcKHXS1JetFjU5");

#[program]
pub mod idl_commands_one {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
