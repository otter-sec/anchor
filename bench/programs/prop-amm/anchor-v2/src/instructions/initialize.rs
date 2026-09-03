use {crate::state::Oracle, anchor_lang::prelude::*};

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer)]
    pub oracle: Account<Oracle>,
    pub system_program: Program<System>,
}
