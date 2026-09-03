use anchor_lang::prelude::*;

declare_id!("Gz3iDiZL332qCU7J2H6yvrXbdeSAeyhUT4Q3ZcLJnb4S");

#[program]
pub mod ix_macro {
    use super::*;

    #[discrim = 0]
    pub fn check_args(
        ctx: &mut Context<CheckArgs>,
        expected_marker: Address,
        amount: u64,
        tag: [u8; 4],
    ) -> Result<()> {
        let marker = ctx.accounts.marker.account().address();
        if !anchor_lang::address_eq(marker, &expected_marker) || tag != *b"ixok" {
            return Err(ProgramError::InvalidInstructionData.into());
        }

        let mut out_view = ctx.accounts.out.account().clone();
        let data = unsafe { out_view.borrow_unchecked_mut() };
        data[..8].copy_from_slice(&amount.to_le_bytes());
        data[8..12].copy_from_slice(&tag);
        data[12..44].copy_from_slice(expected_marker.as_ref());
        data[44] = ctx.bumps.marker_seeded;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(expected_marker: Address, amount: u64, tag: [u8; 4])]
pub struct CheckArgs {
    #[account(address = expected_marker)]
    pub marker: UncheckedAccount,
    #[account(
        seeds = [b"marker", expected_marker.as_ref()],
        bump,
    )]
    pub marker_seeded: UncheckedAccount,
    #[account(mut, constraint = amount == 0x0102_0304_0506_0708 && tag == *b"ixok")]
    pub out: UncheckedAccount,
}
