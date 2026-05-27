//! Associated Token Account address derivation and CPI helpers.
//!
//! Users can validate ATA accounts via `constraint = expr`:
//! ```ignore
//! #[account(
//!     token::mint = mint,
//!     token::authority = authority,
//!     constraint = *vault.account().address() == anchor_spl_v2::get_associated_token_address_with_program_id(
//!         authority.account().address(),
//!         mint.account().address(),
//!         &Token::id(),
//!     )
//! )]
//! pub vault: Account<TokenAccount>,
//! ```

extern crate alloc;

use {
    alloc::{vec, vec::Vec},
    anchor_lang_v2::{programs::Token, CpiContext, CpiHandle, Id, ToCpiAccounts},
    pinocchio::instruction::InstructionAccount,
    solana_address::Address,
    solana_program_error::ProgramError,
};

pub use anchor_lang_v2::programs::AssociatedToken;

pub const ID: Address = Address::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

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

pub struct Create<'a> {
    pub payer: CpiHandle<'a>,
    pub associated_token: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub system_program: CpiHandle<'a>,
    pub token_program: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Create<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.payer.address()),
            InstructionAccount::writable(self.associated_token.address()),
            InstructionAccount::readonly(self.authority.address()),
            InstructionAccount::readonly(self.mint.address()),
            InstructionAccount::readonly(self.system_program.address()),
            InstructionAccount::readonly(self.token_program.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![
            self.payer,
            self.associated_token,
            self.authority,
            self.mint,
            self.system_program,
            self.token_program,
        ]
    }
}

pub type CreateIdempotent<'a> = Create<'a>;

pub fn create<'a>(ctx: CpiContext<'a, Create<'a>>) -> Result<(), ProgramError> {
    #[cfg(feature = "guardrails")]
    {
        if !anchor_lang_v2::address_eq(ctx.program, &AssociatedToken::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        if !anchor_lang_v2::address_eq(
            ctx.accounts.system_program.address(),
            &anchor_lang_v2::programs::System::id(),
        ) {
            return Err(ProgramError::IncorrectProgramId);
        }
    }
    crate::token_shared::validate_token_interface_program(ctx.accounts.token_program.address())?;
    ctx.invoke(&[0])
}

pub fn create_idempotent<'a>(
    ctx: CpiContext<'a, CreateIdempotent<'a>>,
) -> Result<(), ProgramError> {
    #[cfg(feature = "guardrails")]
    {
        if !anchor_lang_v2::address_eq(ctx.program, &AssociatedToken::id()) {
            return Err(ProgramError::IncorrectProgramId);
        }
        if !anchor_lang_v2::address_eq(
            ctx.accounts.system_program.address(),
            &anchor_lang_v2::programs::System::id(),
        ) {
            return Err(ProgramError::IncorrectProgramId);
        }
    }
    crate::token_shared::validate_token_interface_program(ctx.accounts.token_program.address())?;
    ctx.invoke(&[1])
}
