use anchor_lang::prelude::*;

declare_id!("BF748KR4UhPq7xbhFQYd7yFKmh5UYdqed9GbD6oZvEyu");

pub const OTHER_PROGRAM: Address =
    anchor_lang::address!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[account]
pub struct Vault {
    pub value: u64,
    pub authority: Address,
    pub bump: u8,
    pub _pad: [u8; 7],
}

#[account]
pub struct UserState {
    pub value: u64,
}

#[account]
pub struct Config {
    pub program_id: Address,
}

#[account]
pub struct DynamicProgramPda {
    pub value: u64,
}

#[account]
pub struct MacroProgramPda {
    pub value: u64,
}

macro_rules! wrap_program_expr {
    ($expr:expr) => {
        $expr
    };
}

#[program]
pub mod client_builders {
    use super::*;

    #[discrim = 0]
    pub fn initialize_vault(ctx: &mut Context<InitializeVault>) -> Result<()> {
        ctx.accounts.vault.value = 0;
        ctx.accounts.vault.authority = *ctx.accounts.authority.address();
        ctx.accounts.vault.bump = ctx.bumps.vault;
        ctx.accounts.vault._pad = [0; 7];
        Ok(())
    }

    #[discrim = 1]
    pub fn set_value(ctx: &mut Context<SetValue>, value: u64) -> Result<()> {
        ctx.accounts.vault.value = value;
        Ok(())
    }

    #[discrim = 2]
    pub fn set_with_dynamic_args(ctx: &mut Context<SetValue>, label: [u8; 2]) -> Result<()> {
        if label != *b"ok" {
            return Err(ProgramError::InvalidInstructionData.into());
        }
        ctx.accounts.vault.value = 202;
        Ok(())
    }

    #[discrim = 3]
    pub fn touch_program_markers(_ctx: &mut Context<TouchProgramMarkers>) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn optional_builder_case(ctx: &mut Context<OptionalBuilderCase>) -> Result<()> {
        if let Some(user_state) = ctx.accounts.user_state.as_mut() {
            user_state.value = user_state.value.saturating_add(1);
        }
        Ok(())
    }

    #[discrim = 8]
    pub fn optional_derivable_builder_case(
        _ctx: &mut Context<OptionalDerivableBuilderCase>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn check_external_pda(_ctx: &mut Context<CheckExternalPda>) -> Result<()> {
        Ok(())
    }

    #[discrim = 6]
    pub fn check_dynamic_program_pda(_ctx: &mut Context<CheckDynamicProgramPda>) -> Result<()> {
        Ok(())
    }

    #[discrim = 7]
    pub fn check_macro_program_pda(_ctx: &mut Context<CheckMacroProgramPda>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeVault {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"vault", authority.address().as_ref()],
        bump,
    )]
    pub vault: Account<Vault>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct SetValue {
    #[account(mut, seeds = [b"vault", authority.address().as_ref()], bump)]
    pub vault: Account<Vault>,
    #[account(address = vault.authority)]
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct TouchProgramMarkers {
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct OptionalBuilderCase {
    #[account(mut)]
    pub user_state: Option<Account<UserState>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct OptionalDerivableBuilderCase {
    pub system_program: Option<Program<System>>,
    #[account(seeds = [b"optional_pda"], bump)]
    pub optional_pda: Option<UncheckedAccount>,
}

#[derive(Accounts)]
pub struct CheckExternalPda {
    #[account(seeds = [b"external"], bump, seeds::program = OTHER_PROGRAM)]
    pub external_pda: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckDynamicProgramPda {
    pub config: Account<Config>,
    #[account(seeds = [b"other"], bump, seeds::program = config.program_id)]
    pub dynamic_pda: Account<DynamicProgramPda>,
}

#[derive(Accounts)]
pub struct CheckMacroProgramPda {
    pub config: Account<Config>,
    #[account(
        seeds = [b"macro"],
        bump,
        seeds::program = wrap_program_expr!(config.program_id)
    )]
    pub macro_pda: Account<MacroProgramPda>,
}
