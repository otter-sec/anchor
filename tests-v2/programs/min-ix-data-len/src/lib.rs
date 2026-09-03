use anchor_lang::prelude::*;

declare_id!("5s2e6TBgh2AYCEmW3DZi7WJYtNaLWS7M3e8dnNh4qLVA");

#[program]
pub mod min_ix_data_len {
    use super::*;

    #[discrim = 0]
    pub fn short_args(_ctx: &mut Context<Noop>, a: u8, b: u64) -> Result<()> {
        if a != 7 || b != 0x0102_0304_0506_0708 {
            return Err(ProgramError::InvalidInstructionData.into());
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
