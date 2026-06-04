//! An example of a program with token extensions enabled
//!
//! This program is intended to implement various token2022 extensions
//!
//! <https://spl.solana.com/token-2022/extensions>

use anchor_lang::prelude::*;

pub mod instructions;
pub mod utils;
pub use {instructions::*, utils::*};

declare_id!("tKEkkQtgMXhdaz5NMTR3XbdUu215sZyHSj6Menvous1");

#[program]
pub mod token_extensions {
    use super::*;

    pub fn create_mint_account(
        ctx: Context<CreateMintAccount>,
        args: CreateMintAccountArgs,
    ) -> Result<()> {
        instructions::handler(ctx, args)
    }

    pub fn check_mint_extensions_constraints(
        _ctx: Context<CheckMintExtensionConstraints>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn create_group_pointer_mint(_ctx: Context<CreateGroupPointerMint>) -> Result<()> {
        Ok(())
    }

    pub fn update_group_pointer(
        ctx: Context<UpdateGroupPointer>,
        new_group_address: Option<Pubkey>,
    ) -> Result<()> {
        instructions::update_group_pointer_handler(ctx, new_group_address)
    }

    pub fn check_toggle_pause(ctx: Context<CheckTogglePause>) -> Result<()> {
        instructions::toggle_pause_handler(ctx)
    }

    pub fn check_pausable_authority_constraint(
        ctx: Context<CheckPausableAuthorityConstraint>,
    ) -> Result<()> {
        instructions::check_pausable_authority_constraint_handler(ctx)
    }

    pub fn update_and_remove_token_metadata(
        ctx: Context<UpdateAndRemoveTokenMetadata>,
    ) -> Result<()> {
        instructions::update_and_remove_token_metadata_handler(ctx)
    }

    pub fn cpi_create_native_mint(ctx: Context<CpiCreateNativeMint>) -> Result<()> {
        instructions::cpi_create_native_mint_handler(ctx)
    }

    pub fn cpi_initialize_non_transferable_mint(
        ctx: Context<CpiInitializeNonTransferableMint>,
    ) -> Result<()> {
        instructions::cpi_initialize_non_transferable_mint_handler(ctx)
    }
}
