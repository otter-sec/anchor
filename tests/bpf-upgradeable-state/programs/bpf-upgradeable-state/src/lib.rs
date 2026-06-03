use anchor_lang::prelude::*;

use crate::program::BpfUpgradeableState;

declare_id!("Cum9tTyj5HwcEiAmhgaS7Bbj4UczCwsucrCkxRECzM4e");

#[program]
pub mod bpf_upgradeable_state {
    use super::*;
    pub fn set_admin_settings(ctx: Context<SetAdminSettings>, admin_data: u64) -> Result<()> {
        match *ctx.accounts.program {
            UpgradeableLoaderState::Program {
                programdata_address,
            } => {
                if programdata_address != ctx.accounts.program_data.key() {
                    return err!(CustomError::InvalidProgramDataAddress);
                }
            }
            _ => {
                return err!(CustomError::AccountNotProgram);
            }
        };
        ctx.accounts.settings.admin_data = admin_data;
        Ok(())
    }

    pub fn set_admin_settings_use_program_state(
        ctx: Context<SetAdminSettingsUseProgramState>,
        admin_data: u64,
    ) -> Result<()> {
        ctx.accounts.settings.admin_data = admin_data;
        Ok(())
    }
}

#[account]
pub struct Settings {
    admin_data: u64,
}

impl Settings {
    pub const LEN: usize = 8;
}

#[error_code]
pub enum CustomError {
    InvalidProgramDataAddress,
    AccountNotProgram,
    AccountNotBpfUpgradableProgram,
}

#[derive(Accounts)]
pub struct SetAdminSettings<'info> {
    // In a real program, this should be a PDA,
    // so the authority cannot create multiple settings accounts.
    // Not done here for easier testing
    #[account(init, payer = authority, space = Settings::LEN + 8)]
    pub settings: Account<'info, Settings>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(address = crate::ID)]
    pub program: Account<'info, UpgradeableLoaderState>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdminSettingsUseProgramState<'info> {
    // In a real program, this should be a PDA,
    // so the authority cannot create multiple settings accounts.
    // Not done here for easier testing
    #[account(init, payer = authority, space = Settings::LEN + 8)]
    pub settings: Account<'info, Settings>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()))]
    pub program: Program<'info, BpfUpgradeableState>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}
