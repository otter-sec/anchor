//! Token-2022 CPI helpers and interface re-exports.

extern crate alloc;

use {
    alloc::{string::String, vec, vec::Vec},
    anchor_lang_v2::{CpiContext, CpiHandle, ToCpiAccounts},
    pinocchio::{address::Address, instruction::InstructionAccount},
    solana_program_error::ProgramError,
};

pub use anchor_lang_v2::programs::Token2022;
pub use spl_token_2022_interface::{self as spl_token_2022, extension::ExtensionType, ID};

pub struct Transfer<'a> {
    pub from: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Transfer<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.from.address()),
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.to, self.authority]
    }
}

pub struct TransferChecked<'a> {
    pub from: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferChecked<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.from.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.mint, self.to, self.authority]
    }
}

pub struct MintTo<'a> {
    pub mint: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for MintTo<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.to, self.authority]
    }
}

pub struct MintToChecked<'a> {
    pub mint: CpiHandle<'a>,
    pub to: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for MintToChecked<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.to, self.authority]
    }
}

pub struct Burn<'a> {
    pub mint: CpiHandle<'a>,
    pub from: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Burn<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.from.address()),
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.mint, self.authority]
    }
}

pub struct BurnChecked<'a> {
    pub mint: CpiHandle<'a>,
    pub from: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for BurnChecked<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.from.address()),
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.from, self.mint, self.authority]
    }
}

pub struct Approve<'a> {
    pub to: CpiHandle<'a>,
    pub delegate: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Approve<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::new(self.delegate.address(), false, false),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.to, self.delegate, self.authority]
    }
}

pub struct ApproveChecked<'a> {
    pub to: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub delegate: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for ApproveChecked<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.to.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::new(self.delegate.address(), false, false),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.to, self.mint, self.delegate, self.authority]
    }
}

pub struct Revoke<'a> {
    pub source: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Revoke<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.source.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.source, self.authority]
    }
}

pub struct InitializeAccount<'a> {
    pub account: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::new(self.authority.address(), false, false),
            InstructionAccount::new(self.rent.address(), false, false),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.mint, self.authority, self.rent]
    }
}

pub struct InitializeAccount3<'a> {
    pub account: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeAccount3<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::new(self.mint.address(), false, false),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.mint]
    }
}

pub struct CloseAccount<'a> {
    pub account: CpiHandle<'a>,
    pub destination: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CloseAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::writable(self.destination.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.destination, self.authority]
    }
}

pub struct FreezeAccount<'a> {
    pub account: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for FreezeAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.mint, self.authority]
    }
}

pub struct ThawAccount<'a> {
    pub account: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for ThawAccount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.mint, self.authority]
    }
}

pub struct InitializeMint<'a> {
    pub mint: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeMint<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::new(self.rent.address(), false, false),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.rent]
    }
}

pub struct InitializeMint2<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeMint2<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct SetAuthority<'a> {
    pub current_authority: CpiHandle<'a>,
    pub account_or_mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for SetAuthority<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account_or_mint.address()),
            InstructionAccount::readonly_signer(self.current_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account_or_mint, self.current_authority]
    }
}

pub struct SyncNative<'a> {
    pub account: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for SyncNative<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.account.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account]
    }
}

pub struct GetAccountDataSize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for GetAccountDataSize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::new(self.mint.address(), false, false)]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct InitializeMintCloseAuthority<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeMintCloseAuthority<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct InitializeImmutableOwner<'a> {
    pub account: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeImmutableOwner<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.account.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account]
    }
}

pub struct AmountToUiAmount<'a> {
    pub account: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for AmountToUiAmount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::new(
            self.account.address(),
            false,
            false,
        )]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account]
    }
}

pub struct UiAmountToAmount<'a> {
    pub account: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for UiAmountToAmount<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::new(
            self.account.address(),
            false,
            false,
        )]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account]
    }
}

pub struct Reallocate<'a> {
    pub account: CpiHandle<'a>,
    pub payer: CpiHandle<'a>,
    pub system_program: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for Reallocate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::writable_signer(self.payer.address()),
            InstructionAccount::new(self.system_program.address(), false, false),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![
            self.account,
            self.payer,
            self.system_program,
            self.authority,
        ]
    }
}

pub struct WithdrawExcessLamports<'a> {
    pub source: CpiHandle<'a>,
    pub destination: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for WithdrawExcessLamports<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.source.address()),
            InstructionAccount::writable(self.destination.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.source, self.destination, self.authority]
    }
}

pub struct CreateNativeMint<'a> {
    pub payer: CpiHandle<'a>,
    pub native_mint: CpiHandle<'a>,
    pub system_program: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CreateNativeMint<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable_signer(self.payer.address()),
            InstructionAccount::writable(self.native_mint.address()),
            InstructionAccount::new(self.system_program.address(), false, false),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.payer, self.native_mint, self.system_program]
    }
}

pub struct InitializeNonTransferableMint<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InitializeNonTransferableMint<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct PermanentDelegateInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for PermanentDelegateInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

fn return_data_from(program: &Address) -> Result<Vec<u8>, ProgramError> {
    let (return_program, data) = anchor_lang_v2::solana_program::program::get_return_data()
        .ok_or(ProgramError::InvalidInstructionData)?;
    if return_program.to_bytes().as_slice() != program.as_ref() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(data)
}

pub fn transfer<'a>(ctx: CpiContext<'a, Transfer<'a>>, amount: u64) -> Result<(), ProgramError> {
    #[allow(deprecated)]
    let ix = spl_token_2022::instruction::transfer(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_checked<'a>(
    ctx: CpiContext<'a, TransferChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::transfer_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_to<'a>(ctx: CpiContext<'a, MintTo<'a>>, amount: u64) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::mint_to(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_to_checked<'a>(
    ctx: CpiContext<'a, MintToChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::mint_to_checked(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn burn<'a>(ctx: CpiContext<'a, Burn<'a>>, amount: u64) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::burn(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn burn_checked<'a>(
    ctx: CpiContext<'a, BurnChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::burn_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn approve<'a>(ctx: CpiContext<'a, Approve<'a>>, amount: u64) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::approve(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn approve_checked<'a>(
    ctx: CpiContext<'a, ApproveChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::approve_checked(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn revoke<'a>(ctx: CpiContext<'a, Revoke<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::revoke(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_account<'a>(
    ctx: CpiContext<'a, InitializeAccount<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_account3<'a>(
    ctx: CpiContext<'a, InitializeAccount3<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account3(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn close_account<'a>(ctx: CpiContext<'a, CloseAccount<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::close_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn freeze_account<'a>(ctx: CpiContext<'a, FreezeAccount<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::freeze_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn thaw_account<'a>(ctx: CpiContext<'a, ThawAccount<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::thaw_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_mint<'a>(
    ctx: CpiContext<'a, InitializeMint<'a>>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_mint2<'a>(
    ctx: CpiContext<'a, InitializeMint2<'a>>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint2(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn set_authority<'a>(
    ctx: CpiContext<'a, SetAuthority<'a>>,
    authority_type: spl_token_2022::instruction::AuthorityType,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::set_authority(
        ctx.program,
        ctx.accounts.account_or_mint.address(),
        new_authority,
        authority_type,
        ctx.accounts.current_authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn sync_native<'a>(ctx: CpiContext<'a, SyncNative<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::sync_native(ctx.program, ctx.accounts.account.address())?;
    ctx.invoke_ix(ix)
}

pub fn get_account_data_size<'a>(
    ctx: CpiContext<'a, GetAccountDataSize<'a>>,
    extension_types: &[ExtensionType],
) -> Result<u64, ProgramError> {
    let ix = spl_token_2022::instruction::get_account_data_size(
        ctx.program,
        ctx.accounts.mint.address(),
        extension_types,
    )?;
    ctx.invoke_ix(ix)?;
    let data = return_data_from(ctx.program)?;
    let bytes: [u8; 8] = data
        .as_slice()
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn initialize_mint_close_authority<'a>(
    ctx: CpiContext<'a, InitializeMintCloseAuthority<'a>>,
    close_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint_close_authority(
        ctx.program,
        ctx.accounts.mint.address(),
        close_authority,
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_immutable_owner<'a>(
    ctx: CpiContext<'a, InitializeImmutableOwner<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_immutable_owner(
        ctx.program,
        ctx.accounts.account.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn amount_to_ui_amount<'a>(
    ctx: CpiContext<'a, AmountToUiAmount<'a>>,
    amount: u64,
) -> Result<String, ProgramError> {
    let ix = spl_token_2022::instruction::amount_to_ui_amount(
        ctx.program,
        ctx.accounts.account.address(),
        amount,
    )?;
    ctx.invoke_ix(ix)?;
    String::from_utf8(return_data_from(ctx.program)?)
        .map_err(|_| ProgramError::InvalidInstructionData)
}

pub fn ui_amount_to_amount<'a>(
    ctx: CpiContext<'a, UiAmountToAmount<'a>>,
    ui_amount: &str,
) -> Result<u64, ProgramError> {
    let ix = spl_token_2022::instruction::ui_amount_to_amount(
        ctx.program,
        ctx.accounts.account.address(),
        ui_amount,
    )?;
    ctx.invoke_ix(ix)?;
    let data = return_data_from(ctx.program)?;
    let bytes: [u8; 8] = data
        .as_slice()
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn reallocate<'a>(
    ctx: CpiContext<'a, Reallocate<'a>>,
    extension_types: &[ExtensionType],
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::reallocate(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.payer.address(),
        ctx.accounts.authority.address(),
        &[],
        extension_types,
    )?;
    ctx.invoke_ix(ix)
}

pub fn withdraw_excess_lamports<'a>(
    ctx: CpiContext<'a, WithdrawExcessLamports<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::withdraw_excess_lamports(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn create_native_mint<'a>(
    ctx: CpiContext<'a, CreateNativeMint<'a>>,
) -> Result<(), ProgramError> {
    let ix =
        spl_token_2022::instruction::create_native_mint(ctx.program, ctx.accounts.payer.address())?;
    ctx.invoke_ix(ix)
}

pub fn initialize_non_transferable_mint<'a>(
    ctx: CpiContext<'a, InitializeNonTransferableMint<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_non_transferable_mint(
        ctx.program,
        ctx.accounts.mint.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_permanent_delegate<'a>(
    ctx: CpiContext<'a, PermanentDelegateInitialize<'a>>,
    permanent_delegate: &Address,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_permanent_delegate(
        ctx.program,
        ctx.accounts.mint.address(),
        permanent_delegate,
    )?;
    ctx.invoke_ix(ix)
}
