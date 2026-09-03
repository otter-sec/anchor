use anchor_lang::prelude::*;

declare_id!("C4Rjm3f8WgNfqszebK3qEsE5neUwSv1a5kJcmWiVfVS3");

#[account]
pub struct Fresh {
    pub value: u64,
}

#[program]
pub mod account_meta_signer_overrides {
    use super::*;

    #[discrim = 0]
    pub fn require_signer(_ctx: &mut Context<RequireSigner>) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn nested_signer(_ctx: &mut Context<NestedSignerOuter>) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn inner_signer(_ctx: &mut Context<NestedSignerInner>) -> Result<()> {
        Ok(())
    }

    #[discrim = 3]
    pub fn init_keypair(_ctx: &mut Context<InitKeypair>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct RequireSigner {
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct NestedSignerInner {
    #[account(mut)]
    pub writable: UncheckedAccount,
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct NestedSignerOuter {
    pub inner: Nested<NestedSignerInner>,
    pub spectator: UncheckedAccount,
}

#[derive(Accounts)]
pub struct InitKeypair {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer)]
    pub fresh: Account<Fresh>,
    pub system_program: Program<System>,
}
