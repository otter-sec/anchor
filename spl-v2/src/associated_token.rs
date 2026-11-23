//! Associated Token Account address derivation and CPI helpers.
//!
//! Users can validate ATA accounts via `associated_token::*` constraints:
//! ```ignore
//! #[account(
//!     associated_token::mint = mint,
//!     associated_token::authority = authority,
//!     associated_token::token_program = token_program,
//! )]
//! pub vault: Account<TokenAccount>,
//! ```

extern crate alloc;

#[cfg(feature = "guardrails")]
use anchor_lang::require;
use {
    anchor_lang::{programs::Token, CpiContext, CpiHandle, CpiHandleMut, Id, ToCpiAccounts},
    solana_address::Address,
    solana_program_error::ProgramError,
};

pub use anchor_lang::programs::AssociatedToken;

pub const ID: Address = anchor_lang::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// Derive the associated token account address for a given wallet and mint.
pub fn get_associated_token_address(wallet: &Address, mint: &Address) -> Address {
    get_associated_token_address_with_program_id(wallet, mint, &Token::id())
}

/// Derive the associated token account address for a given wallet, mint, and token program.
pub fn get_associated_token_address_with_program_id(
    wallet: &Address,
    mint: &Address,
    token_program_id: &Address,
) -> Address {
    let seeds: &[&[u8]] = &[wallet.as_ref(), token_program_id.as_ref(), mint.as_ref()];
    let (addr, _bump) = Address::find_program_address(seeds, &ID);
    addr
}

#[derive(ToCpiAccounts)]
pub struct Create<'a> {
    #[signer]
    pub payer: CpiHandleMut<'a>,
    pub associated_token: CpiHandleMut<'a>,
    pub authority: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub system_program: CpiHandle<'a>,
    pub token_program: CpiHandle<'a>,
}

pub type CreateIdempotent<'a> = Create<'a>;

pub fn create<'a>(ctx: CpiContext<'a, Create<'a>>) -> Result<(), ProgramError> {
    #[cfg(feature = "guardrails")]
    {
        require!(
            anchor_lang::address_eq(ctx.program, &AssociatedToken::id()),
            ProgramError::IncorrectProgramId
        );
        require!(
            anchor_lang::address_eq(
                ctx.accounts.system_program.address(),
                &anchor_lang::programs::System::id(),
            ),
            ProgramError::IncorrectProgramId
        );
    }
    crate::token_shared::validate_token_interface_program(ctx.accounts.token_program.address())?;
    ctx.invoke(&[0])
}

pub fn create_idempotent<'a>(
    ctx: CpiContext<'a, CreateIdempotent<'a>>,
) -> Result<(), ProgramError> {
    #[cfg(feature = "guardrails")]
    {
        require!(
            anchor_lang::address_eq(ctx.program, &AssociatedToken::id()),
            ProgramError::IncorrectProgramId
        );
        require!(
            anchor_lang::address_eq(
                ctx.accounts.system_program.address(),
                &anchor_lang::programs::System::id(),
            ),
            ProgramError::IncorrectProgramId
        );
    }
    crate::token_shared::validate_token_interface_program(ctx.accounts.token_program.address())?;
    ctx.invoke(&[1])
}
