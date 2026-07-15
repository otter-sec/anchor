use anchor_lang::prelude::*;

declare_id!("Dec1areProgram11111111111111111111111111111");

declare_program!(external);
use external::program::External;

// Compilation check for legacy IDL (pre Anchor `0.30`)
declare_program!(external_legacy);

// Compilation check for the Raydium AMM v3 program
// https://github.com/raydium-io/raydium-idl/blob/6123104304ebcb42be175cc297a2c221ac96bb96/raydium_clmm/amm_v3.json
declare_program!(amm_v3);

// Compilation check for an IDL type named after a prelude item: `Key` mirrors
// mpl-core's account discriminator, which any IDL for a program that CPIs
// into mpl-core embeds, and collides with the `anchor_lang::Key` trait (#4775)
declare_program!(mpl_core_shape);

#[program]
pub mod declare_program {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>, value: u32) -> Result<()> {
        let cpi_my_account = &mut ctx.accounts.cpi_my_account;
        require_keys_eq!(external::accounts::MyAccount::owner(), external::ID);
        require_eq!(cpi_my_account.field, 0);

        let cpi_ctx = CpiContext::new(
            ctx.accounts.external_program.key(),
            external::cpi::accounts::Update {
                authority: ctx.accounts.authority.to_account_info(),
                my_account: cpi_my_account.to_account_info(),
            },
        );
        external::cpi::update(cpi_ctx, value)?;

        cpi_my_account.reload()?;
        require_eq!(cpi_my_account.field, value);

        Ok(())
    }

    pub fn cpi_composite(ctx: Context<Cpi>, value: u32) -> Result<()> {
        let cpi_my_account = &mut ctx.accounts.cpi_my_account;

        // Composite accounts that's also an instruction
        let cpi_ctx = CpiContext::new(
            ctx.accounts.external_program.key(),
            external::cpi::accounts::UpdateComposite {
                update: external::cpi::accounts::Update {
                    authority: ctx.accounts.authority.to_account_info(),
                    my_account: cpi_my_account.to_account_info(),
                },
            },
        );
        external::cpi::update_composite(cpi_ctx, 42)?;
        cpi_my_account.reload()?;
        require_eq!(cpi_my_account.field, 42);

        // Composite accounts but not an actual instruction
        let cpi_ctx = CpiContext::new(
            ctx.accounts.external_program.key(),
            external::cpi::accounts::UpdateNonInstructionComposite {
                non_instruction_update: external::cpi::accounts::NonInstructionUpdate {
                    authority: ctx.accounts.authority.to_account_info(),
                    my_account: cpi_my_account.to_account_info(),
                    program: ctx.accounts.external_program.to_account_info(),
                },
            },
        );
        external::cpi::update_non_instruction_composite(cpi_ctx, 10)?;
        cpi_my_account.reload()?;
        require_eq!(cpi_my_account.field, 10);

        // Composite accounts but not an actual instruction (intentionally checking multiple times)
        let cpi_ctx = CpiContext::new(
            ctx.accounts.external_program.key(),
            external::cpi::accounts::UpdateNonInstructionComposite2 {
                non_instruction_update: external::cpi::accounts::NonInstructionUpdate2 {
                    program: ctx.accounts.external_program.to_account_info(),
                },
                non_instruction_update_with_different_ident:
                    external::cpi::accounts::NonInstructionUpdate {
                        authority: ctx.accounts.authority.to_account_info(),
                        my_account: cpi_my_account.to_account_info(),
                        program: ctx.accounts.external_program.to_account_info(),
                    },
            },
        );
        external::cpi::update_non_instruction_composite2(cpi_ctx, value)?;
        cpi_my_account.reload()?;
        require_eq!(cpi_my_account.field, value);

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Cpi<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub cpi_my_account: Account<'info, external::accounts::MyAccount>,
    pub external_program: Program<'info, External>,
}
