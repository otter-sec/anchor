use anchor_lang::prelude::*;

declare_id!("9NxAd91hhJ3ZBTHytYP894y4ESRKG7n8VbLgdyYGJFLB");

#[program]
pub mod native_system {
    use super::*;

    pub fn create_account(
        ctx: Context<CreateAccount>,
        lamports: u64,
        space: u64,
        owner: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn assign(ctx: Context<Assign>, owner: Pubkey) -> Result<()> {
        Ok(())
    }

    pub fn transfer(ctx: Context<Transfer>, lamports: u64) -> Result<()> {
        Ok(())
    }

    pub fn create_account_with_seed(
        ctx: Context<CreateAccountWithSeed>,
        base: Pubkey,
        seed: String,
        lamports: u64,
        space: u64,
        owner: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn advance_nonce_account(
        ctx: Context<AdvanceNonceAccount>,
        authorized: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn withdraw_nonce_account(ctx: Context<WithdrawNonceAccount>, lamports: u64) -> Result<()> {
        Ok(())
    }

    pub fn initialize_nonce_account(
        ctx: Context<InitializeNonceAccount>,
        authorized: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn authorize_nonce_account(
        ctx: Context<AuthorizeNonceAccount>,
        authorized: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn allocate(ctx: Context<Allocate>, space: u64) -> Result<()> {
        Ok(())
    }

    pub fn allocate_with_seed(
        ctx: Context<AllocateWithSeed>,
        base: Pubkey,
        seed: String,
        space: u64,
        owner: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn assign_with_seed(
        ctx: Context<AssignWithSeed>,
        base: Pubkey,
        seed: String,
        owner: Pubkey,
    ) -> Result<()> {
        Ok(())
    }

    pub fn transfer_with_seed(
        ctx: Context<TransferWithSeed>,
        lamports: u64,
        seed: String,
        owner: Pubkey,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateAccount<'info> {
    #[account(mut)]
    from: Signer<'info>,
    #[account(mut)]
    to: Signer<'info>,
}

#[derive(Accounts)]
pub struct Assign<'info> {
    #[account(mut)]
    pubkey: Signer<'info>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    from: Signer<'info>,
    #[account(mut)]
    /// CHECK:
    to: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CreateAccountWithSeed<'info> {
    #[account(mut)]
    from: Signer<'info>,
    #[account(mut)]
    /// CHECK:
    to: UncheckedAccount<'info>,
    base: Signer<'info>,
}

#[derive(Accounts)]
pub struct AdvanceNonceAccount<'info> {
    #[account(mut)]
    /// CHECK:
    nonce: UncheckedAccount<'info>,
    /// CHECK:
    recent_blockhashes: UncheckedAccount<'info>,
    authorized: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawNonceAccount<'info> {
    #[account(mut)]
    /// CHECK:
    nonce: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK:
    to: UncheckedAccount<'info>,
    /// CHECK:
    recent_blockhashes: UncheckedAccount<'info>,
    rent: Sysvar<'info, Rent>,
    authorized: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeNonceAccount<'info> {
    #[account(mut)]
    nonce: Signer<'info>,
    /// CHECK:
    recent_blockhashes: UncheckedAccount<'info>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AuthorizeNonceAccount<'info> {
    #[account(mut)]
    /// CHECK:
    nonce: UncheckedAccount<'info>,
    authorized: Signer<'info>,
}

#[derive(Accounts)]
pub struct Allocate<'info> {
    #[account(mut)]
    pubkey: Signer<'info>,
}

#[derive(Accounts)]
pub struct AllocateWithSeed<'info> {
    #[account(mut)]
    /// CHECK:
    account: UncheckedAccount<'info>,
    base: Signer<'info>,
}

#[derive(Accounts)]
pub struct AssignWithSeed<'info> {
    #[account(mut)]
    /// CHECK:
    account: UncheckedAccount<'info>,
    base: Signer<'info>,
}

#[derive(Accounts)]
pub struct TransferWithSeed<'info> {
    #[account(mut)]
    /// CHECK:
    from: UncheckedAccount<'info>,
    base: Signer<'info>,
    #[account(mut)]
    /// CHECK:
    to: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FeeCalculator {
    pub lamports_per_signature: u64,
}

#[account]
pub struct Nonce {
    pub version: u32,
    pub state: u32,
    pub authorized_pubkey: Pubkey,
    pub nonce: Pubkey,
    pub fee_calculator: FeeCalculator,
}
