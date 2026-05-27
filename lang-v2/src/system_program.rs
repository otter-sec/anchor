//! System program CPI helpers.
//!
//! Mirrors the v1 `anchor_lang::system_program` CPI utility surface while
//! using v2 [`CpiContext`] and [`CpiHandle`] accounts.

extern crate alloc;

use {
    crate::{CpiContext, CpiHandle, Id, ToCpiAccounts},
    alloc::{string::String, vec, vec::Vec},
    pinocchio::{address::MAX_SEED_LEN, instruction::InstructionAccount},
    solana_address::Address,
    solana_program_error::{ProgramError, ProgramResult},
};

pub use crate::programs::System;

pub const ID: Address = Address::from_str_const("11111111111111111111111111111111");

const NONCE_ACCOUNT_LENGTH: u64 = 80;

#[inline]
fn check_system_program(program: &Address) -> ProgramResult {
    if crate::address_eq(program, &System::id()) {
        Ok(())
    } else {
        Err(ProgramError::IncorrectProgramId)
    }
}

#[inline]
fn encode_u32(value: u32, data: &mut Vec<u8>) {
    data.extend_from_slice(&value.to_le_bytes());
}

#[inline]
fn encode_u64(value: u64, data: &mut Vec<u8>) {
    data.extend_from_slice(&value.to_le_bytes());
}

#[inline]
fn encode_address(value: &Address, data: &mut Vec<u8>) {
    data.extend_from_slice(value.as_ref());
}

fn checked_seed(seed: &str) -> Result<&[u8], ProgramError> {
    let seed = seed.as_bytes();
    if seed.len() > MAX_SEED_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(seed)
}

#[inline]
fn invoke<'a, T: ToCpiAccounts<'a>>(ctx: &CpiContext<'a, T>, data: &[u8]) -> ProgramResult {
    check_system_program(ctx.program)?;
    ctx.invoke(data)
}

pub fn advance_nonce_account<'a>(ctx: CpiContext<'a, AdvanceNonceAccount<'a>>) -> ProgramResult {
    invoke(&ctx, &4u32.to_le_bytes())
}

pub struct AdvanceNonceAccount<'a> {
    pub nonce: CpiHandle<'a>,
    pub authorized: CpiHandle<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for AdvanceNonceAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.nonce.address()),
            InstructionAccount::readonly(self.recent_blockhashes.address()),
            InstructionAccount::readonly_signer(self.authorized.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.nonce, self.recent_blockhashes, self.authorized]
    }
}

pub fn allocate<'a>(ctx: CpiContext<'a, Allocate<'a>>, space: u64) -> ProgramResult {
    let mut data = Vec::with_capacity(12);
    encode_u32(8, &mut data);
    encode_u64(space, &mut data);
    invoke(&ctx, &data)
}

pub struct Allocate<'a> {
    pub account_to_allocate: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Allocate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable_signer(
            self.account_to_allocate.address(),
        )]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account_to_allocate]
    }
}

pub fn allocate_with_seed<'a>(
    ctx: CpiContext<'a, AllocateWithSeed<'a>>,
    seed: &str,
    space: u64,
    owner: &Address,
) -> ProgramResult {
    let seed = checked_seed(seed)?;
    let mut data = Vec::with_capacity(84 + seed.len());
    encode_u32(9, &mut data);
    encode_address(ctx.accounts.base.address(), &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_u64(space, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

pub struct AllocateWithSeed<'a> {
    pub account_to_allocate: CpiHandle<'a>,
    pub base: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for AllocateWithSeed<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account_to_allocate.address()),
            InstructionAccount::readonly_signer(self.base.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account_to_allocate, self.base]
    }
}

pub fn assign<'a>(ctx: CpiContext<'a, Assign<'a>>, owner: &Address) -> ProgramResult {
    let mut data = Vec::with_capacity(36);
    encode_u32(1, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

pub struct Assign<'a> {
    pub account_to_assign: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Assign<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable_signer(
            self.account_to_assign.address(),
        )]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account_to_assign]
    }
}

pub fn assign_with_seed<'a>(
    ctx: CpiContext<'a, AssignWithSeed<'a>>,
    seed: &str,
    owner: &Address,
) -> ProgramResult {
    let seed = checked_seed(seed)?;
    let mut data = Vec::with_capacity(76 + seed.len());
    encode_u32(10, &mut data);
    encode_address(ctx.accounts.base.address(), &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

pub struct AssignWithSeed<'a> {
    pub account_to_assign: CpiHandle<'a>,
    pub base: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for AssignWithSeed<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account_to_assign.address()),
            InstructionAccount::readonly_signer(self.base.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account_to_assign, self.base]
    }
}

pub fn authorize_nonce_account<'a>(
    ctx: CpiContext<'a, AuthorizeNonceAccount<'a>>,
    new_authority: &Address,
) -> ProgramResult {
    let mut data = Vec::with_capacity(36);
    encode_u32(7, &mut data);
    encode_address(new_authority, &mut data);
    invoke(&ctx, &data)
}

pub struct AuthorizeNonceAccount<'a> {
    pub nonce: CpiHandle<'a>,
    pub authorized: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for AuthorizeNonceAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.nonce.address()),
            InstructionAccount::readonly_signer(self.authorized.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.nonce, self.authorized]
    }
}

pub fn create_account<'a>(
    ctx: CpiContext<'a, CreateAccount<'a>>,
    lamports: u64,
    space: u64,
    owner: &Address,
) -> ProgramResult {
    let mut data = Vec::with_capacity(52);
    encode_u32(0, &mut data);
    encode_u64(lamports, &mut data);
    encode_u64(space, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

pub struct CreateAccount<'a> {
    pub from: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CreateAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.from.address()),
            InstructionAccount::writable_signer(self.to.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.to]
    }
}

pub fn create_account_with_seed<'a>(
    ctx: CpiContext<'a, CreateAccountWithSeed<'a>>,
    seed: &str,
    lamports: u64,
    space: u64,
    owner: &Address,
) -> ProgramResult {
    let seed = checked_seed(seed)?;
    let mut data = Vec::with_capacity(92 + seed.len());
    encode_u32(3, &mut data);
    encode_address(ctx.accounts.base.address(), &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_u64(lamports, &mut data);
    encode_u64(space, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

pub struct CreateAccountWithSeed<'a> {
    pub from: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
    pub base: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CreateAccountWithSeed<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.from.address()),
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::readonly_signer(self.base.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.to, self.base]
    }
}

pub fn create_nonce_account<'a>(
    ctx: CpiContext<'a, CreateNonceAccount<'a>>,
    lamports: u64,
    authority: &Address,
) -> ProgramResult {
    let owner = System::id();
    let create_accounts = CreateAccount {
        from: ctx.accounts.from,
        to: ctx.accounts.nonce,
    };
    create_account(
        CpiContext::new_with_signer(ctx.program, create_accounts, ctx.signer_seeds),
        lamports,
        NONCE_ACCOUNT_LENGTH,
        &owner,
    )?;

    let initialize_accounts = InitializeNonceAccount {
        nonce: ctx.accounts.nonce,
        recent_blockhashes: ctx.accounts.recent_blockhashes,
        rent: ctx.accounts.rent,
    };
    initialize_nonce_account(
        CpiContext::new_with_signer(ctx.program, initialize_accounts, ctx.signer_seeds),
        authority,
    )
}

pub struct CreateNonceAccount<'a> {
    pub from: CpiHandle<'a>,
    pub nonce: CpiHandle<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CreateNonceAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.from.address()),
            InstructionAccount::writable_signer(self.nonce.address()),
            InstructionAccount::readonly(self.recent_blockhashes.address()),
            InstructionAccount::readonly(self.rent.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.nonce, self.recent_blockhashes, self.rent]
    }
}

pub fn create_nonce_account_with_seed<'a>(
    ctx: CpiContext<'a, CreateNonceAccountWithSeed<'a>>,
    lamports: u64,
    seed: &str,
    authority: &Address,
) -> ProgramResult {
    let owner = System::id();
    let create_accounts = CreateAccountWithSeed {
        from: ctx.accounts.from,
        to: ctx.accounts.nonce,
        base: ctx.accounts.base,
    };
    create_account_with_seed(
        CpiContext::new_with_signer(ctx.program, create_accounts, ctx.signer_seeds),
        seed,
        lamports,
        NONCE_ACCOUNT_LENGTH,
        &owner,
    )?;

    let initialize_accounts = InitializeNonceAccount {
        nonce: ctx.accounts.nonce,
        recent_blockhashes: ctx.accounts.recent_blockhashes,
        rent: ctx.accounts.rent,
    };
    initialize_nonce_account(
        CpiContext::new_with_signer(ctx.program, initialize_accounts, ctx.signer_seeds),
        authority,
    )
}

pub struct CreateNonceAccountWithSeed<'a> {
    pub from: CpiHandle<'a>,
    pub nonce: CpiHandle<'a>,
    pub base: CpiHandle<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CreateNonceAccountWithSeed<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.from.address()),
            InstructionAccount::writable(self.nonce.address()),
            InstructionAccount::readonly_signer(self.base.address()),
            InstructionAccount::readonly(self.recent_blockhashes.address()),
            InstructionAccount::readonly(self.rent.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![
            self.from,
            self.nonce,
            self.base,
            self.recent_blockhashes,
            self.rent,
        ]
    }
}

pub fn transfer<'a>(ctx: CpiContext<'a, Transfer<'a>>, lamports: u64) -> ProgramResult {
    let mut data = Vec::with_capacity(12);
    encode_u32(2, &mut data);
    encode_u64(lamports, &mut data);
    invoke(&ctx, &data)
}

pub struct Transfer<'a> {
    pub from: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Transfer<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.from.address()),
            InstructionAccount::writable(self.to.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.to]
    }
}

pub fn transfer_with_seed<'a>(
    ctx: CpiContext<'a, TransferWithSeed<'a>>,
    from_seed: String,
    from_owner: &Address,
    lamports: u64,
) -> ProgramResult {
    let seed = checked_seed(&from_seed)?;
    let mut data = Vec::with_capacity(52 + seed.len());
    encode_u32(11, &mut data);
    encode_u64(lamports, &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_address(from_owner, &mut data);
    invoke(&ctx, &data)
}

pub struct TransferWithSeed<'a> {
    pub from: CpiHandle<'a>,
    pub base: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferWithSeed<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.from.address()),
            InstructionAccount::readonly_signer(self.base.address()),
            InstructionAccount::writable(self.to.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.base, self.to]
    }
}

pub fn withdraw_nonce_account<'a>(
    ctx: CpiContext<'a, WithdrawNonceAccount<'a>>,
    lamports: u64,
) -> ProgramResult {
    let mut data = Vec::with_capacity(12);
    encode_u32(5, &mut data);
    encode_u64(lamports, &mut data);
    invoke(&ctx, &data)
}

pub struct WithdrawNonceAccount<'a> {
    pub nonce: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
    pub authorized: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for WithdrawNonceAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.nonce.address()),
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::readonly(self.recent_blockhashes.address()),
            InstructionAccount::readonly(self.rent.address()),
            InstructionAccount::readonly_signer(self.authorized.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![
            self.nonce,
            self.to,
            self.recent_blockhashes,
            self.rent,
            self.authorized,
        ]
    }
}

struct InitializeNonceAccount<'a> {
    nonce: CpiHandle<'a>,
    recent_blockhashes: CpiHandle<'a>,
    rent: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeNonceAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.nonce.address()),
            InstructionAccount::readonly(self.recent_blockhashes.address()),
            InstructionAccount::readonly(self.rent.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.nonce, self.recent_blockhashes, self.rent]
    }
}

fn initialize_nonce_account<'a>(
    ctx: CpiContext<'a, InitializeNonceAccount<'a>>,
    authority: &Address,
) -> ProgramResult {
    let mut data = Vec::with_capacity(36);
    encode_u32(6, &mut data);
    encode_address(authority, &mut data);
    invoke(&ctx, &data)
}
