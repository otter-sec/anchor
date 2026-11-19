//! Examples for `anchor-spl` Mint and TokenAccount workflows.
//!
//! The handlers show common SPL initialization, CPI helpers, account
//! accessors, namespaced constraints, and Token Interface patterns.

use {
    anchor_lang::prelude::*,
    anchor_spl::{
        associated_token::get_associated_token_address,
        extensions::{
            self, CpiGuard, DefaultAccountState, GroupMemberPointer, GroupPointer, MetadataPointer,
            MintCloseAuthority, NonTransferable, NonTransferableAccount, PausableAccount,
            PausableConfig, PermanentDelegate, TransferFeeAmount, TransferFeeConfig, TransferHook,
            TransferHookAccount,
        },
        mint::{self, Mint},
        token::{self, TokenAccount, TokenCpiExt},
        token_2022 as token_2022_cpi, token_2022_extensions as token_2022_ext_cpi,
        token_interface::{self, TokenInterfaceAccountExtensions},
    },
};

declare_id!("SpL1111111111111111111111111111111111111111");

const TOKEN_CPI_EXT_AUTHORITY_SEED: &[u8] = b"token-cpi-ext-authority";

#[program]
pub mod spl_test {
    use super::*;

    /// Creates a new Mint account with the standard mint initialization
    /// constraint.
    #[discrim = 0]
    pub fn init_mint(_ctx: &mut Context<InitMint>) -> Result<()> {
        Ok(())
    }

    /// Creates a new TokenAccount with the standard token initialization
    /// constraint.
    #[discrim = 1]
    pub fn init_token_account(_ctx: &mut Context<InitTokenAccount>) -> Result<()> {
        Ok(())
    }

    /// Mints `amount` tokens into `to`.
    #[discrim = 2]
    pub fn do_mint_to(ctx: &mut Context<DoMintTo>, amount: u64) -> Result<()> {
        ctx.accounts.token_program.mint_to(
            &mut ctx.accounts.mint,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            &[],
            amount,
        )?;
        Ok(())
    }

    /// Transfers `amount` tokens from `from` to `to`.
    #[discrim = 3]
    pub fn do_transfer(ctx: &mut Context<DoTransfer>, amount: u64) -> Result<()> {
        ctx.accounts.token_program.transfer(
            &mut ctx.accounts.from,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            &[],
            amount,
        )?;
        Ok(())
    }

    /// Transfers tokens while verifying the mint decimals.
    #[discrim = 4]
    pub fn do_transfer_checked(
        ctx: &mut Context<DoTransferChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.token_program.transfer_checked(
            &mut ctx.accounts.from,
            &ctx.accounts.mint,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            &[],
            amount,
            decimals,
        )?;
        Ok(())
    }

    /// Burns `amount` tokens from `account`.
    #[discrim = 5]
    pub fn do_burn(ctx: &mut Context<DoBurn>, amount: u64) -> Result<()> {
        ctx.accounts.token_program.burn(
            &mut ctx.accounts.account,
            &mut ctx.accounts.mint,
            &ctx.accounts.authority,
            &[],
            amount,
        )?;
        Ok(())
    }

    /// Approves `delegate` to spend `amount` from `source`.
    #[discrim = 6]
    pub fn do_approve(ctx: &mut Context<DoApprove>, amount: u64) -> Result<()> {
        ctx.accounts.token_program.approve(
            &mut ctx.accounts.source,
            &ctx.accounts.delegate,
            &ctx.accounts.authority,
            &[],
            amount,
        )?;
        Ok(())
    }

    /// Revokes the current delegate from `source`.
    #[discrim = 7]
    pub fn do_revoke(ctx: &mut Context<DoRevoke>) -> Result<()> {
        ctx.accounts.token_program.revoke(
            &mut ctx.accounts.source,
            &ctx.accounts.authority,
            &[],
        )?;
        Ok(())
    }

    /// Closes `account`, reclaiming lamports to `destination`.
    #[discrim = 8]
    pub fn do_close_account(ctx: &mut Context<DoCloseAccount>) -> Result<()> {
        ctx.accounts.token_program.close_account(
            &mut ctx.accounts.account,
            &mut ctx.accounts.destination,
            &ctx.accounts.authority,
            &[],
        )?;

        Ok(())
    }

    /// MintToChecked verifies both the mint authority and declared decimals.
    #[discrim = 77]
    pub fn do_mint_to_checked(
        ctx: &mut Context<DoMintToChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.token_program.mint_to_checked(
            &mut ctx.accounts.mint,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            &[],
            amount,
            decimals,
        )?;
        Ok(())
    }

    /// BurnChecked verifies both the token owner and declared decimals.
    #[discrim = 78]
    pub fn do_burn_checked(
        ctx: &mut Context<DoBurnChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.token_program.burn_checked(
            &mut ctx.accounts.account,
            &mut ctx.accounts.mint,
            &ctx.accounts.authority,
            &[],
            amount,
            decimals,
        )?;
        Ok(())
    }

    /// ApproveChecked verifies delegate allowance against the mint decimals.
    #[discrim = 79]
    pub fn do_approve_checked(
        ctx: &mut Context<DoApproveChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.token_program.approve_checked(
            &mut ctx.accounts.source,
            &ctx.accounts.mint,
            &ctx.accounts.delegate,
            &ctx.accounts.authority,
            &[],
            amount,
            decimals,
        )?;
        Ok(())
    }

    /// Freeze and thaw cover Token account state transitions driven by the
    /// mint's freeze authority.
    #[discrim = 80]
    pub fn do_freeze_account(ctx: &mut Context<DoFreezeAccount>) -> Result<()> {
        ctx.accounts.token_program.freeze_account(
            &mut ctx.accounts.account,
            &ctx.accounts.mint,
            &ctx.accounts.authority,
            &[],
        )?;
        Ok(())
    }

    #[discrim = 81]
    pub fn do_thaw_account(ctx: &mut Context<DoThawAccount>) -> Result<()> {
        ctx.accounts.token_program.thaw_account(
            &mut ctx.accounts.account,
            &ctx.accounts.mint,
            &ctx.accounts.authority,
            &[],
        )?;
        Ok(())
    }

    /// SyncNative updates a wrapped SOL account's token amount from lamports.
    #[discrim = 82]
    pub fn do_sync_native(ctx: &mut Context<DoSyncNative>) -> Result<()> {
        ctx.accounts
            .token_program
            .sync_native(&mut ctx.accounts.account)?;
        Ok(())
    }

    /// Direct CPI helpers initialize token accounts and mints that have
    /// already been created.
    #[discrim = 83]
    pub fn do_initialize_mint(
        ctx: &mut Context<DoInitializeMint>,
        decimals: u8,
        authority: Address,
    ) -> Result<()> {
        ctx.accounts.token_program.initialize_mint(
            &mut ctx.accounts.mint,
            &ctx.accounts.rent,
            decimals,
            &authority,
            None,
        )?;
        Ok(())
    }

    #[discrim = 84]
    pub fn do_initialize_mint2(
        ctx: &mut Context<DoInitializeMint2>,
        decimals: u8,
        authority: Address,
    ) -> Result<()> {
        ctx.accounts.token_program.initialize_mint2(
            &mut ctx.accounts.mint,
            decimals,
            &authority,
            None,
        )?;
        Ok(())
    }

    #[discrim = 85]
    pub fn do_initialize_account(ctx: &mut Context<DoInitializeAccount>) -> Result<()> {
        ctx.accounts.token_program.initialize_account(
            &mut ctx.accounts.account,
            &ctx.accounts.mint,
            &ctx.accounts.authority,
            &ctx.accounts.rent,
        )?;
        Ok(())
    }

    #[discrim = 86]
    pub fn do_initialize_account3(ctx: &mut Context<DoInitializeAccount3>) -> Result<()> {
        ctx.accounts.token_program.initialize_account3(
            &mut ctx.accounts.account,
            &ctx.accounts.mint,
            &ctx.accounts.authority,
        )?;
        Ok(())
    }

    /// Set close authority through the legacy `token::set_authority` shim.
    #[discrim = 87]
    pub fn do_set_close_authority(
        ctx: &mut Context<DoSetAuthority>,
        new_authority: Address,
    ) -> Result<()> {
        ctx.accounts.token_program.set_authority(
            &mut ctx.accounts.account_or_mint,
            &ctx.accounts.current_authority,
            &[],
            token::spl_token::instruction::AuthorityType::CloseAccount,
            Some(new_authority),
        )?;
        Ok(())
    }

    /// MintTo through TokenCpiExt using a PDA authority.
    #[discrim = 93]
    pub fn do_mint_to_pda_authority(
        ctx: &mut Context<DoMintToPdaAuthority>,
        amount: u64,
    ) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.mint_to(
            &mut ctx.accounts.mint,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            signer_seeds,
            amount,
        )?;
        Ok(())
    }

    /// Transfer through TokenCpiExt using a PDA token-account owner.
    #[discrim = 94]
    pub fn do_transfer_pda_authority(
        ctx: &mut Context<DoTransferPdaAuthority>,
        amount: u64,
    ) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.transfer(
            &mut ctx.accounts.from,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            signer_seeds,
            amount,
        )?;
        Ok(())
    }

    /// Set close authority through TokenCpiExt using a PDA current authority.
    #[discrim = 95]
    pub fn do_set_close_authority_pda(
        ctx: &mut Context<DoSetAuthorityPda>,
        new_authority: Address,
    ) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.set_authority(
            &mut ctx.accounts.account_or_mint,
            &ctx.accounts.authority,
            signer_seeds,
            token::spl_token::instruction::AuthorityType::CloseAccount,
            Some(new_authority),
        )?;
        Ok(())
    }

    /// TransferChecked through TokenCpiExt using a PDA token-account owner.
    #[discrim = 96]
    pub fn do_transfer_checked_pda_authority(
        ctx: &mut Context<DoTransferCheckedPdaAuthority>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.transfer_checked(
            &mut ctx.accounts.from,
            &ctx.accounts.mint,
            &mut ctx.accounts.to,
            &ctx.accounts.authority,
            signer_seeds,
            amount,
            decimals,
        )?;
        Ok(())
    }

    /// Burn through TokenCpiExt using a PDA token-account owner.
    #[discrim = 97]
    pub fn do_burn_pda_authority(
        ctx: &mut Context<DoBurnPdaAuthority>,
        amount: u64,
    ) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.burn(
            &mut ctx.accounts.account,
            &mut ctx.accounts.mint,
            &ctx.accounts.authority,
            signer_seeds,
            amount,
        )?;
        Ok(())
    }

    /// Approve through TokenCpiExt using a PDA token-account owner.
    #[discrim = 98]
    pub fn do_approve_pda_authority(
        ctx: &mut Context<DoApprovePdaAuthority>,
        amount: u64,
    ) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.approve(
            &mut ctx.accounts.source,
            &ctx.accounts.delegate,
            &ctx.accounts.authority,
            signer_seeds,
            amount,
        )?;
        Ok(())
    }

    /// Thaw through TokenCpiExt using a PDA freeze authority.
    #[discrim = 99]
    pub fn do_thaw_account_pda_authority(ctx: &mut Context<DoThawAccountPdaAuthority>) -> Result<()> {
        let bump = [ctx.bumps.authority];
        let authority_seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED, &bump];
        let signer_seeds = &[&authority_seeds[..]];
        ctx.accounts.token_program.thaw_account(
            &mut ctx.accounts.account,
            &ctx.accounts.mint,
            &ctx.accounts.authority,
            signer_seeds,
        )?;
        Ok(())
    }

    /// Reads every `Mint` accessor: supply, decimals, authority flags, and
    /// freeze-authority fields.
    #[discrim = 9]
    pub fn read_mint(
        ctx: &mut Context<ReadMint>,
        expected_authority: Address,
        expected_supply: u64,
        expected_decimals: u8,
    ) -> Result<()> {
        let m = &*ctx.accounts.mint;
        require_eq!(
            m.supply(),
            expected_supply,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            m.decimals(),
            expected_decimals,
            ProgramError::InvalidAccountData
        );
        require!(m.has_mint_authority(), ProgramError::InvalidAccountData);
        require_eq!(
            m.mint_authority(),
            Some(&expected_authority),
            ProgramError::InvalidAccountData
        );
        require!(m.is_initialized(), ProgramError::InvalidAccountData);
        Ok(())
    }

    /// Reads every `TokenAccount` accessor: balances, delegate fields, state,
    /// native SOL fields, and close authority.
    #[discrim = 10]
    pub fn read_token_account(
        ctx: &mut Context<ReadTokenAccount>,
        expected_mint: Address,
        expected_owner: Address,
        expected_amount: u64,
    ) -> Result<()> {
        let ta = &*ctx.accounts.token_account;
        require!(
            anchor_lang::address_eq(ta.mint(), &expected_mint),
            ProgramError::InvalidAccountData
        );
        require!(
            anchor_lang::address_eq(ta.owner(), &expected_owner),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ta.amount(),
            expected_amount,
            ProgramError::InvalidAccountData
        );
        require!(ta.is_initialized(), ProgramError::InvalidAccountData);
        Ok(())
    }

    /// Assert every Mint accessor against a deliberately non-default state.
    #[discrim = 88]
    pub fn assert_mint_accessors(
        ctx: &mut Context<ReadMint>,
        expected_authority: Address,
        expected_freeze_authority: Address,
        expected_supply: u64,
        expected_decimals: u8,
    ) -> Result<()> {
        let m = &*ctx.accounts.mint;
        require_eq!(
            m.supply(),
            expected_supply,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            m.decimals(),
            expected_decimals,
            ProgramError::InvalidAccountData
        );
        require!(m.has_mint_authority(), ProgramError::InvalidAccountData);
        require_eq!(
            m.mint_authority(),
            Some(&expected_authority),
            ProgramError::InvalidAccountData
        );
        require!(m.is_initialized(), ProgramError::InvalidAccountData);
        require!(m.has_freeze_authority(), ProgramError::InvalidAccountData);
        require_eq!(
            m.freeze_authority(),
            Some(&expected_freeze_authority),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Assert every TokenAccount accessor against a deliberately rich state:
    /// delegate, close authority, native reserve, and frozen status.
    #[discrim = 89]
    pub fn assert_token_account_accessors(
        ctx: &mut Context<ReadTokenAccount>,
        expected_mint: Address,
        expected_owner: Address,
        expected_amount: u64,
        expected_delegate: Address,
        expected_delegated_amount: u64,
        expected_close_authority: Address,
        expected_native_amount: u64,
    ) -> Result<()> {
        let ta = &*ctx.accounts.token_account;
        require!(
            anchor_lang::address_eq(ta.mint(), &expected_mint),
            ProgramError::InvalidAccountData
        );
        require!(
            anchor_lang::address_eq(ta.owner(), &expected_owner),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ta.amount(),
            expected_amount,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ta.delegated_amount(),
            expected_delegated_amount,
            ProgramError::InvalidAccountData
        );
        require!(ta.has_delegate(), ProgramError::InvalidAccountData);
        require_eq!(
            ta.delegate(),
            Some(&expected_delegate),
            ProgramError::InvalidAccountData
        );
        require_eq!(ta.state(), 2, ProgramError::InvalidAccountData);
        require!(ta.is_native(), ProgramError::InvalidAccountData);
        require_eq!(
            ta.native_amount(),
            Some(expected_native_amount),
            ProgramError::InvalidAccountData
        );
        require!(ta.has_close_authority(), ProgramError::InvalidAccountData);
        require_eq!(
            ta.close_authority(),
            Some(&expected_close_authority),
            ProgramError::InvalidAccountData
        );
        require!(ta.is_initialized(), ProgramError::InvalidAccountData);
        require!(ta.is_frozen(), ProgramError::InvalidAccountData);
        Ok(())
    }

    /// `mint::decimals = 6` constraint. Tests pass a mint with matching
    /// decimals; mismatch path asserts the `InvalidAccountData` response.
    #[discrim = 11]
    pub fn check_mint_decimals(_ctx: &mut Context<CheckMintDecimals>) -> Result<()> {
        Ok(())
    }

    /// `mint::authority = expected` constraint.
    #[discrim = 12]
    pub fn check_mint_authority(_ctx: &mut Context<CheckMintAuthority>) -> Result<()> {
        Ok(())
    }

    /// `mint::freeze_authority = expected` constraint on strict `Account<Mint>`.
    #[discrim = 90]
    pub fn check_mint_freeze_authority(_ctx: &mut Context<CheckMintFreezeAuthority>) -> Result<()> {
        Ok(())
    }

    /// `mint::token_program = token_program` constraint on strict `Account<Mint>`.
    #[discrim = 91]
    pub fn check_mint_token_program(_ctx: &mut Context<CheckMintTokenProgram>) -> Result<()> {
        Ok(())
    }

    /// `token::mint = mint` constraint.
    #[discrim = 13]
    pub fn check_token_mint(_ctx: &mut Context<CheckTokenMint>) -> Result<()> {
        Ok(())
    }

    /// `token::authority = expected` constraint.
    #[discrim = 14]
    pub fn check_token_authority(_ctx: &mut Context<CheckTokenAuthority>) -> Result<()> {
        Ok(())
    }

    /// `token::token_program = token_program` constraint on strict `Account<TokenAccount>`.
    #[discrim = 92]
    pub fn check_token_program(_ctx: &mut Context<CheckTokenProgram>) -> Result<()> {
        Ok(())
    }

    /// Verifies that `vault` is the canonical ATA for `(authority, mint)`.
    #[discrim = 15]
    pub fn check_ata(ctx: &mut Context<CheckAta>) -> Result<()> {
        let expected = get_associated_token_address(
            ctx.accounts.authority.account().address(),
            ctx.accounts.mint.account().address(),
        );
        require!(
            anchor_lang::address_eq(ctx.accounts.vault.account().address(), &expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    // ---- InterfaceAccount validation --------------------------------------

    /// Validates an `InterfaceAccount<Mint>` owned by either Token or
    /// Token-2022 against the expected mint state.
    #[discrim = 16]
    pub fn read_interface_mint(
        ctx: &mut Context<ReadInterfaceMint>,
        expected_authority: Address,
        expected_supply: u64,
        expected_decimals: u8,
    ) -> Result<()> {
        let m = &*ctx.accounts.mint;
        require_eq!(
            m.supply(),
            expected_supply,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            m.decimals(),
            expected_decimals,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            m.mint_authority(),
            Some(&expected_authority),
            ProgramError::InvalidAccountData
        );
        require!(m.is_initialized(), ProgramError::InvalidAccountData);
        Ok(())
    }

    /// Validates an `InterfaceAccount<TokenAccount>` against the expected
    /// token account state.
    #[discrim = 17]
    pub fn read_interface_token_account(
        ctx: &mut Context<ReadInterfaceTokenAccount>,
        expected_mint: Address,
        expected_owner: Address,
        expected_amount: u64,
    ) -> Result<()> {
        let ta = &*ctx.accounts.token_account;
        require!(
            anchor_lang::address_eq(ta.mint(), &expected_mint),
            ProgramError::InvalidAccountData
        );
        require!(
            anchor_lang::address_eq(ta.owner(), &expected_owner),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ta.amount(),
            expected_amount,
            ProgramError::InvalidAccountData
        );
        require!(ta.is_initialized(), ProgramError::InvalidAccountData);
        Ok(())
    }

    // ---- InterfaceAccount initialization ----------------------------------

    /// Create a new Mint through the `InterfaceAccount<Mint>` init path with
    /// the legacy Token program (InitializeMint2 is hardcoded to legacy).
    #[discrim = 18]
    pub fn init_interface_mint(_ctx: &mut Context<InitInterfaceMint>) -> Result<()> {
        Ok(())
    }

    /// Create a new TokenAccount through the `InterfaceAccount<TokenAccount>`
    /// init path with the legacy Token program.
    #[discrim = 19]
    pub fn init_interface_token_account(
        _ctx: &mut Context<InitInterfaceTokenAccount>,
    ) -> Result<()> {
        Ok(())
    }

    // ---- Namespaced constraints on InterfaceAccount -----------------------

    /// `token::mint = mint` on `InterfaceAccount<TokenAccount>`.
    #[discrim = 20]
    pub fn check_interface_token_mint(_ctx: &mut Context<CheckInterfaceTokenMint>) -> Result<()> {
        Ok(())
    }

    /// `token::authority = expected` on `InterfaceAccount<TokenAccount>`.
    #[discrim = 21]
    pub fn check_interface_token_authority(
        _ctx: &mut Context<CheckInterfaceTokenAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    /// `token::token_program = token_program` on `InterfaceAccount<TokenAccount>`.
    #[discrim = 22]
    pub fn check_interface_token_program(
        _ctx: &mut Context<CheckInterfaceTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    /// `mint::authority = expected` on `InterfaceAccount<Mint>`.
    #[discrim = 23]
    pub fn check_interface_mint_authority(
        _ctx: &mut Context<CheckInterfaceMintAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    /// `mint::freeze_authority = expected` on `InterfaceAccount<Mint>`.
    #[discrim = 24]
    pub fn check_interface_mint_freeze_authority(
        _ctx: &mut Context<CheckInterfaceMintFreezeAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    /// `mint::decimals = 6` on `InterfaceAccount<Mint>`.
    #[discrim = 25]
    pub fn check_interface_mint_decimals(
        _ctx: &mut Context<CheckInterfaceMintDecimals>,
    ) -> Result<()> {
        Ok(())
    }

    /// `mint::token_program = token_program` on `InterfaceAccount<Mint>`.
    #[discrim = 26]
    pub fn check_interface_mint_token_program(
        _ctx: &mut Context<CheckInterfaceMintTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    // ---- Token-2022 extension parsing -------------------------------------

    /// Parse `TransferFeeConfig` from a Token-2022 mint and assert the
    /// newer transfer fee's basis-points value matches `expected_bps`.
    #[discrim = 27]
    pub fn read_transfer_fee_config(
        ctx: &mut Context<ReadTransferFeeConfig>,
        expected_withheld: u64,
        expected_older_epoch: u64,
        expected_older_max_fee: u64,
        expected_older_bps: u16,
        expected_newer_epoch: u64,
        expected_newer_max_fee: u64,
        expected_newer_bps: u16,
    ) -> Result<()> {
        let ext: &TransferFeeConfig = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.withheld_amount(),
            expected_withheld,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.older_transfer_fee.epoch(),
            expected_older_epoch,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.older_transfer_fee.maximum_fee(),
            expected_older_max_fee,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.older_transfer_fee.basis_points(),
            expected_older_bps,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.newer_transfer_fee.epoch(),
            expected_newer_epoch,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.newer_transfer_fee.maximum_fee(),
            expected_newer_max_fee,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.newer_transfer_fee.basis_points(),
            expected_newer_bps,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `TransferFeeConfig` through the `InterfaceAccount<Mint>` extension
    /// reader trait.
    #[discrim = 55]
    pub fn read_transfer_fee_config_via_trait(
        ctx: &mut Context<ReadTransferFeeConfigViaTrait>,
        expected_bps: u16,
    ) -> Result<()> {
        let ext: &TransferFeeConfig = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.newer_transfer_fee.basis_points(),
            expected_bps,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `MetadataPointer` and assert both the `authority` and
    /// `metadata_address` fields match the passed pubkeys.
    #[discrim = 28]
    pub fn read_metadata_pointer(
        ctx: &mut Context<ReadMetadataPointer>,
        expected_authority: Address,
        expected_metadata: Address,
    ) -> Result<()> {
        let ext: &MetadataPointer = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.authority,
            expected_authority,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.metadata_address,
            expected_metadata,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `TransferHook` and assert the `program_id` matches.
    #[discrim = 29]
    pub fn read_transfer_hook(
        ctx: &mut Context<ReadTransferHook>,
        expected_program_id: Address,
    ) -> Result<()> {
        let ext: &TransferHook = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.program_id,
            expected_program_id,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `MintCloseAuthority`, exercising `optional_address` on the
    /// resulting field — returns an error when the close authority is unset.
    #[discrim = 30]
    pub fn read_mint_close_authority(
        ctx: &mut Context<ReadMintCloseAuthority>,
        expected_authority: Address,
        expect_none: u8,
    ) -> Result<()> {
        let ext: &MintCloseAuthority = ctx.accounts.mint.get_extension()?;
        let authority = extensions::optional_address(&ext.close_authority);
        if expect_none != 0 {
            require!(authority.is_none(), ProgramError::InvalidAccountData);
        } else {
            require_eq!(
                authority,
                Some(&expected_authority),
                ProgramError::InvalidAccountData
            );
        }
        Ok(())
    }

    /// Parse `PermanentDelegate` and compare via `optional_address`.
    #[discrim = 31]
    pub fn read_permanent_delegate(
        ctx: &mut Context<ReadPermanentDelegate>,
        expected_delegate: Address,
        expect_none: u8,
    ) -> Result<()> {
        let ext: &PermanentDelegate = ctx.accounts.mint.get_extension()?;
        let delegate = extensions::optional_address(&ext.delegate);
        if expect_none != 0 {
            require!(delegate.is_none(), ProgramError::InvalidAccountData);
        } else {
            require_eq!(
                delegate,
                Some(&expected_delegate),
                ProgramError::InvalidAccountData
            );
        }
        Ok(())
    }

    /// Parse `TransferFeeAmount` from a Token-2022 token account.
    #[discrim = 32]
    pub fn read_transfer_fee_amount(
        ctx: &mut Context<ReadTransferFeeAmount>,
        expected_withheld: u64,
    ) -> Result<()> {
        let ext: &TransferFeeAmount = ctx.accounts.token_account.get_extension()?;
        require_eq!(
            ext.withheld_amount(),
            expected_withheld,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `TransferFeeAmount` through the `InterfaceAccount<TokenAccount>`
    /// extension reader trait.
    #[discrim = 56]
    pub fn read_transfer_fee_amount_via_trait(
        ctx: &mut Context<ReadTransferFeeAmountViaTrait>,
        expected_withheld: u64,
    ) -> Result<()> {
        let ext: &TransferFeeAmount = ctx.accounts.token_account.get_extension()?;
        require_eq!(
            ext.withheld_amount(),
            expected_withheld,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `TransferHookAccount` from a Token-2022 token account.
    #[discrim = 33]
    pub fn read_transfer_hook_account(
        ctx: &mut Context<ReadTransferHookAccount>,
        expected_transferring: u8,
    ) -> Result<()> {
        let ext: &TransferHookAccount = ctx.accounts.token_account.get_extension()?;
        require_eq!(
            ext.transferring,
            expected_transferring,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `DefaultAccountState` from a Token-2022 mint.
    #[discrim = 34]
    pub fn read_default_account_state(
        ctx: &mut Context<ReadDefaultAccountState>,
        expected_state: u8,
    ) -> Result<()> {
        let ext: &DefaultAccountState = ctx.accounts.mint.get_extension()?;
        require_eq!(ext.state, expected_state, ProgramError::InvalidAccountData);
        Ok(())
    }

    /// Parse `GroupPointer` and assert both optional addresses match.
    #[discrim = 35]
    pub fn read_group_pointer(
        ctx: &mut Context<ReadGroupPointer>,
        expected_authority: Address,
        expected_group: Address,
    ) -> Result<()> {
        let ext: &GroupPointer = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.authority,
            expected_authority,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.group_address,
            expected_group,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `GroupMemberPointer` and assert both optional addresses match.
    #[discrim = 36]
    pub fn read_group_member_pointer(
        ctx: &mut Context<ReadGroupMemberPointer>,
        expected_authority: Address,
        expected_member: Address,
    ) -> Result<()> {
        let ext: &GroupMemberPointer = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.authority,
            expected_authority,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ext.member_address,
            expected_member,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `CpiGuard` from a Token-2022 token account.
    #[discrim = 37]
    pub fn read_cpi_guard(ctx: &mut Context<ReadCpiGuard>, expected_enabled: u8) -> Result<()> {
        let ext: &CpiGuard = ctx.accounts.token_account.get_extension()?;
        require_eq!(
            u8::from(ext.is_enabled()),
            expected_enabled,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse `PausableConfig` from a Token-2022 mint.
    #[discrim = 38]
    pub fn read_pausable_config(
        ctx: &mut Context<ReadPausableConfig>,
        expected_authority: Address,
        expected_paused: u8,
    ) -> Result<()> {
        let ext: &PausableConfig = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.authority,
            expected_authority,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            u8::from(ext.is_paused()),
            expected_paused,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse zero-sized mint/account marker extensions.
    #[discrim = 39]
    pub fn read_marker_extensions(ctx: &mut Context<ReadMarkerExtensions>) -> Result<()> {
        let _: &NonTransferable = ctx.accounts.mint.get_extension()?;
        let _: &NonTransferableAccount = ctx.accounts.token_account.get_extension()?;
        let _: &PausableAccount = ctx.accounts.token_account.get_extension()?;
        Ok(())
    }

    /// Parse a mint extension. Used to assert that
    /// `InterfaceAccount<Mint>::get_extension` performs its own validation.
    #[discrim = 45]
    pub fn read_unchecked_transfer_fee_config(
        ctx: &mut Context<ReadUncheckedMintExtension>,
        expected_bps: u16,
    ) -> Result<()> {
        let ext: &TransferFeeConfig = ctx.accounts.mint.get_extension()?;
        require_eq!(
            ext.newer_transfer_fee.basis_points(),
            expected_bps,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Parse a token account extension. Used to assert that
    /// `InterfaceAccount<TokenAccount>::get_extension` validates base
    /// Token-2022 account shape before walking TLV.
    #[discrim = 46]
    pub fn read_unchecked_transfer_fee_amount(
        ctx: &mut Context<ReadUncheckedTokenAccountExtension>,
        expected_withheld: u64,
    ) -> Result<()> {
        let ext: &TransferFeeAmount = ctx.accounts.token_account.get_extension()?;
        require_eq!(
            ext.withheld_amount(),
            expected_withheld,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Attempt to parse an account extension from a mint-shaped unchecked
    /// account. This must fail through SPL Token-2022's extension-family check.
    #[discrim = 53]
    pub fn read_unchecked_mint_transfer_fee_amount(
        ctx: &mut Context<ReadUncheckedMintTransferFeeAmount>,
    ) -> Result<()> {
        let _: &TransferFeeAmount = ctx.accounts.mint.get_extension()?;
        Ok(())
    }

    /// Attempt to parse a mint extension from a token-account-shaped unchecked
    /// account. This must fail through SPL Token-2022's extension-family check.
    #[discrim = 54]
    pub fn read_unchecked_token_account_transfer_fee_config(
        ctx: &mut Context<ReadUncheckedTokenAccountTransferFeeConfig>,
    ) -> Result<()> {
        let _: &TransferFeeConfig = ctx.accounts.token_account.get_extension()?;
        Ok(())
    }

    /// Invoke the Token-2022 group pointer update helper against the supplied Token-2022 program.
    #[discrim = 41]
    pub fn token_2022_group_pointer_update(
        ctx: &mut Context<Token2022GroupPointerUpdate>,
        group_address: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::GroupPointerUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::group_pointer_update(cpi_ctx, Some(&group_address))?;
        Ok(())
    }

    /// Invoke the Token-2022 group member pointer update helper against the supplied Token-2022 program.
    #[discrim = 42]
    pub fn token_2022_group_member_pointer_update(
        ctx: &mut Context<Token2022GroupMemberPointerUpdate>,
        member_address: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::GroupMemberPointerUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::group_member_pointer_update(cpi_ctx, Some(&member_address))?;
        Ok(())
    }

    /// Invoke the Token-2022 reallocate helper against the supplied Token-2022 program.
    #[discrim = 43]
    pub fn token_2022_reallocate_immutable_owner(
        ctx: &mut Context<Token2022Reallocate>,
    ) -> Result<()> {
        let accs = token_2022_cpi::Reallocate {
            account: ctx.accounts.account.cpi_handle_mut(),
            payer: ctx.accounts.payer.cpi_handle_mut(),
            system_program: ctx.accounts.system_program.cpi_handle(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::reallocate(cpi_ctx, &[token_2022_cpi::ExtensionType::ImmutableOwner])?;

        Ok(())
    }

    /// Invoke the direct Token-2022 immutable owner helper against the supplied Token-2022 program.
    #[discrim = 68]
    pub fn direct_token_2022_immutable_owner_initialize(
        ctx: &mut Context<Token2022ImmutableOwnerInitialize>,
    ) -> Result<()> {
        let accs = token_2022_cpi::InitializeImmutableOwner {
            account: ctx.accounts.token_account.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::initialize_immutable_owner(cpi_ctx)?;
        Ok(())
    }

    /// Invoke the direct Token-2022 mint close authority helper against the supplied Token-2022 program.
    #[discrim = 69]
    pub fn direct_token_2022_mint_close_authority_initialize(
        ctx: &mut Context<Token2022MintCloseAuthorityInitialize>,
        close_authority: Address,
    ) -> Result<()> {
        let accs = token_2022_cpi::InitializeMintCloseAuthority {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::initialize_mint_close_authority(cpi_ctx, Some(&close_authority))?;
        Ok(())
    }

    /// Invoke the direct Token-2022 non-transferable mint helper against the supplied Token-2022 program.
    #[discrim = 70]
    pub fn direct_token_2022_non_transferable_mint_initialize(
        ctx: &mut Context<Token2022NonTransferableMintInitialize>,
    ) -> Result<()> {
        let accs = token_2022_cpi::InitializeNonTransferableMint {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::initialize_non_transferable_mint(cpi_ctx)?;
        Ok(())
    }

    /// Invoke the direct Token-2022 permanent delegate helper against the supplied Token-2022 program.
    #[discrim = 71]
    pub fn direct_token_2022_permanent_delegate_initialize(
        ctx: &mut Context<Token2022PermanentDelegateInitialize>,
        permanent_delegate: Address,
    ) -> Result<()> {
        let accs = token_2022_cpi::PermanentDelegateInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::initialize_permanent_delegate(cpi_ctx, &permanent_delegate)?;
        Ok(())
    }

    /// Invoke the direct Token-2022 get-account-data-size helper and assert its return data.
    #[discrim = 72]
    pub fn direct_token_2022_get_account_data_size(
        ctx: &mut Context<Token2022ReturnDataMint>,
    ) -> Result<()> {
        let accs = token_2022_cpi::GetAccountDataSize {
            mint: ctx.accounts.mint.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        let size = token_2022_cpi::get_account_data_size(
            cpi_ctx,
            &[token_2022_cpi::ExtensionType::ImmutableOwner],
        )?;
        require_eq!(
            size,
            token_2022_cpi::ExtensionType::try_calculate_account_len::<
                token_2022_cpi::spl_token_2022::state::Account,
            >(&[token_2022_cpi::ExtensionType::ImmutableOwner])
            .map_err(|_| ProgramError::InvalidInstructionData)? as u64,
            ProgramError::InvalidInstructionData
        );
        Ok(())
    }

    /// Invoke the direct Token-2022 amount-to-ui-amount helper and assert its return data.
    #[discrim = 73]
    pub fn direct_token_2022_amount_to_ui_amount(
        ctx: &mut Context<Token2022ReturnDataMint>,
    ) -> Result<()> {
        let accs = token_2022_cpi::AmountToUiAmount {
            account: ctx.accounts.mint.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        let ui_amount = token_2022_cpi::amount_to_ui_amount(cpi_ctx, 123456789)?;
        require!(
            ui_amount.as_bytes() == b"123.456789",
            ProgramError::InvalidInstructionData
        );
        Ok(())
    }

    /// Invoke the direct Token-2022 ui-amount-to-amount helper and assert its return data.
    #[discrim = 75]
    pub fn direct_token_2022_ui_amount_to_amount(
        ctx: &mut Context<Token2022ReturnDataMint>,
    ) -> Result<()> {
        let accs = token_2022_cpi::UiAmountToAmount {
            account: ctx.accounts.mint.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        let amount = token_2022_cpi::ui_amount_to_amount(cpi_ctx, "42.125")?;
        require_eq!(amount, 42125000u64, ProgramError::InvalidInstructionData);
        Ok(())
    }

    /// Invoke Token-2022 immutable owner initialization against the supplied Token-2022 program.
    #[discrim = 57]
    pub fn token_2022_immutable_owner_initialize(
        ctx: &mut Context<Token2022ImmutableOwnerInitialize>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::ImmutableOwnerInitialize {
            token_account: ctx.accounts.token_account.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::immutable_owner_initialize(cpi_ctx)?;
        Ok(())
    }

    /// Invoke Token-2022 non-transferable mint initialization against the supplied Token-2022 program.
    #[discrim = 58]
    pub fn token_2022_non_transferable_mint_initialize(
        ctx: &mut Context<Token2022NonTransferableMintInitialize>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::NonTransferableMintInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::non_transferable_mint_initialize(cpi_ctx)?;
        Ok(())
    }

    /// Invoke Token-2022 mint close authority initialization against the supplied Token-2022 program.
    #[discrim = 59]
    pub fn token_2022_mint_close_authority_initialize(
        ctx: &mut Context<Token2022MintCloseAuthorityInitialize>,
        close_authority: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::MintCloseAuthorityInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::mint_close_authority_initialize(cpi_ctx, Some(&close_authority))?;
        Ok(())
    }

    /// Invoke Token-2022 permanent delegate initialization against the supplied Token-2022 program.
    #[discrim = 60]
    pub fn token_2022_permanent_delegate_initialize(
        ctx: &mut Context<Token2022PermanentDelegateInitialize>,
        permanent_delegate: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::PermanentDelegateInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::permanent_delegate_initialize(cpi_ctx, &permanent_delegate)?;
        Ok(())
    }

    /// Invoke Token-2022 default account state initialization against the supplied Token-2022 program.
    #[discrim = 61]
    pub fn token_2022_default_account_state_initialize(
        ctx: &mut Context<Token2022DefaultAccountStateInitialize>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::DefaultAccountStateInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::default_account_state_initialize(
            cpi_ctx,
            &token_2022_cpi::spl_token_2022::state::AccountState::Frozen,
        )?;
        Ok(())
    }

    /// Invoke Token-2022 memo transfer enablement against the supplied Token-2022 program.
    #[discrim = 62]
    pub fn token_2022_memo_transfer_initialize(
        ctx: &mut Context<Token2022MemoTransfer>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::MemoTransfer {
            account: ctx.accounts.account.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::memo_transfer_initialize(cpi_ctx)?;
        Ok(())
    }

    /// Invoke Token-2022 metadata pointer initialization against the supplied Token-2022 program.
    #[discrim = 63]
    pub fn token_2022_metadata_pointer_initialize(
        ctx: &mut Context<Token2022MetadataPointerInitialize>,
        authority: Address,
        metadata_address: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::MetadataPointerInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::metadata_pointer_initialize(
            cpi_ctx,
            Some(&authority),
            Some(&metadata_address),
        )?;
        Ok(())
    }

    /// Invoke Token-2022 transfer hook initialization against the supplied Token-2022 program.
    #[discrim = 64]
    pub fn token_2022_transfer_hook_initialize(
        ctx: &mut Context<Token2022TransferHookInitialize>,
        authority: Address,
        hook_program: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TransferHookInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::transfer_hook_initialize(
            cpi_ctx,
            Some(&authority),
            Some(&hook_program),
        )?;
        Ok(())
    }

    /// Invoke Token-2022 interest-bearing mint initialization against the supplied Token-2022 program.
    #[discrim = 65]
    pub fn token_2022_interest_bearing_mint_initialize(
        ctx: &mut Context<Token2022InterestBearingMintInitialize>,
        rate_authority: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::InterestBearingMintInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::interest_bearing_mint_initialize(cpi_ctx, Some(&rate_authority), 125)?;
        Ok(())
    }

    /// Invoke Token-2022 pausable initialization against the supplied Token-2022 program.
    #[discrim = 66]
    pub fn token_2022_pausable_initialize(
        ctx: &mut Context<Token2022PausableInitialize>,
        authority: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::PausableInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::pausable_initialize(cpi_ctx, &authority)?;
        Ok(())
    }

    /// Invoke Token-2022 transfer fee initialization against the supplied Token-2022 program.
    #[discrim = 67]
    pub fn token_2022_transfer_fee_initialize(
        ctx: &mut Context<Token2022TransferFeeInitialize>,
        config_authority: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TransferFeeInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::transfer_fee_initialize(
            cpi_ctx,
            Some(&config_authority),
            None,
            111,
            42,
        )?;
        Ok(())
    }

    /// Invoke Token-2022 native mint creation against the supplied Token-2022 program.
    #[discrim = 100]
    pub fn token_2022_create_native_mint(
        ctx: &mut Context<Token2022CreateNativeMint>,
    ) -> Result<()> {
        let accs = token_2022_cpi::CreateNativeMint {
            payer: ctx.accounts.payer.cpi_handle_mut(),
            native_mint: ctx.accounts.native_mint.cpi_handle_mut(),
            system_program: ctx.accounts.system_program.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::create_native_mint(cpi_ctx)?;
        Ok(())
    }

    /// Invoke Token-2022 excess lamport withdrawal against the supplied Token-2022 program.
    #[discrim = 101]
    pub fn token_2022_withdraw_excess_lamports(
        ctx: &mut Context<Token2022WithdrawExcessLamports>,
    ) -> Result<()> {
        let accs = token_2022_cpi::WithdrawExcessLamports {
            source: ctx.accounts.source.cpi_handle_mut(),
            destination: ctx.accounts.destination.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::withdraw_excess_lamports(cpi_ctx)?;
        Ok(())
    }

    /// Invoke Token Metadata remove_key against the supplied Token-2022 program.
    #[discrim = 47]
    pub fn token_2022_token_metadata_remove_key(
        ctx: &mut Context<Token2022TokenMetadataRemoveKey>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMetadataRemoveKey {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_metadata_remove_key(cpi_ctx, "field".into(), true)?;
        Ok(())
    }

    /// Invoke Token Metadata initialize against the supplied Token-2022 program.
    #[discrim = 48]
    pub fn token_2022_token_metadata_initialize(
        ctx: &mut Context<Token2022TokenMetadataInitialize>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMetadataInitialize {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
            mint_authority: ctx.accounts.mint_authority.cpi_handle(),
            mint: ctx.accounts.mint.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_metadata_initialize(
            cpi_ctx,
            "name".into(),
            "SYM".into(),
            "https://example.invalid".into(),
        )?;
        Ok(())
    }

    /// Invoke Token Metadata update_authority against the supplied Token-2022 program.
    #[discrim = 49]
    pub fn token_2022_token_metadata_update_authority(
        ctx: &mut Context<Token2022TokenMetadataUpdateAuthority>,
        new_authority: Address,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMetadataUpdateAuthority {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            current_authority: ctx.accounts.current_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_metadata_update_authority(cpi_ctx, Some(&new_authority))?;
        Ok(())
    }

    /// Invoke Token Metadata update_field against the supplied Token-2022 program.
    #[discrim = 50]
    pub fn token_2022_token_metadata_update_field(
        ctx: &mut Context<Token2022TokenMetadataUpdateField>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMetadataUpdateField {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_metadata_update_field(
            cpi_ctx,
            token_2022_ext_cpi::token_metadata::Field::Name,
            "name".into(),
        )?;
        Ok(())
    }

    /// Invoke Token Group initialize against the supplied Token-2022 program.
    #[discrim = 51]
    pub fn token_2022_token_group_initialize(
        ctx: &mut Context<Token2022TokenGroupInitialize>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenGroupInitialize {
            group: ctx.accounts.group.cpi_handle_mut(),
            mint: ctx.accounts.mint.cpi_handle(),
            mint_authority: ctx.accounts.mint_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_group_initialize(cpi_ctx, None, 10)?;
        Ok(())
    }

    /// Invoke Token Group member initialize against the supplied Token-2022 program.
    #[discrim = 52]
    pub fn token_2022_token_member_initialize(
        ctx: &mut Context<Token2022TokenMemberInitialize>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMemberInitialize {
            member: ctx.accounts.member.cpi_handle_mut(),
            member_mint: ctx.accounts.member_mint.cpi_handle(),
            member_mint_authority: ctx.accounts.member_mint_authority.cpi_handle(),
            group: ctx.accounts.group.cpi_handle_mut(),
            group_update_authority: ctx.accounts.group_update_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_member_initialize(cpi_ctx)?;
        Ok(())
    }

    /// Burn through an unchecked token program account. This is intentionally
    /// only used to verify `token::burn` rejects arbitrary CPI targets itself.
    #[discrim = 44]
    pub fn do_burn_unchecked_token_program(
        ctx: &mut Context<DoBurnUncheckedTokenProgram>,
        amount: u64,
    ) -> Result<()> {
        let accs = token::Burn {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            from: ctx.accounts.account.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::burn(cpi_ctx, amount)?;
        Ok(())
    }
}

// -- Accounts structs --------------------------------------------------------
//
// Sibling field refs used in namespaced constraints (e.g. `mint::authority
// = authority`) must appear above the field that references them —
// `try_accounts` loads fields in declaration order and codegen emits the
// Constrain call after all earlier fields have been pulled.

#[derive(Accounts)]
pub struct InitMint {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitTokenAccount {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct DoMintTo {
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoTransfer {
    #[account(mut)]
    pub from: Account<TokenAccount>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoTransferChecked {
    #[account(mut)]
    pub from: Account<TokenAccount>,
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoBurn {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    #[account(mut)]
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoBurnUncheckedTokenProgram {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    #[account(mut)]
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct DoApprove {
    #[account(mut)]
    pub source: Account<TokenAccount>,
    pub delegate: UncheckedAccount,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoRevoke {
    #[account(mut)]
    pub source: Account<TokenAccount>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoCloseAccount {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoMintToChecked {
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoBurnChecked {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    #[account(mut)]
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoApproveChecked {
    #[account(mut)]
    pub source: Account<TokenAccount>,
    pub mint: Account<Mint>,
    pub delegate: UncheckedAccount,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoFreezeAccount {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoThawAccount {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoSyncNative {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoInitializeMint {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub rent: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoInitializeMint2 {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoInitializeAccount {
    #[account(mut)]
    pub account: UncheckedAccount,
    pub mint: UncheckedAccount,
    pub authority: UncheckedAccount,
    pub rent: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoInitializeAccount3 {
    #[account(mut)]
    pub account: UncheckedAccount,
    pub mint: UncheckedAccount,
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoSetAuthority {
    #[account(mut)]
    pub account_or_mint: Account<TokenAccount>,
    pub current_authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoMintToPdaAuthority {
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoTransferPdaAuthority {
    #[account(mut)]
    pub from: Account<TokenAccount>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoSetAuthorityPda {
    #[account(mut)]
    pub account_or_mint: Account<TokenAccount>,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoTransferCheckedPdaAuthority {
    #[account(mut)]
    pub from: Account<TokenAccount>,
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<TokenAccount>,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoBurnPdaAuthority {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoApprovePdaAuthority {
    #[account(mut)]
    pub source: Account<TokenAccount>,
    pub delegate: UncheckedAccount,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DoThawAccountPdaAuthority {
    #[account(mut)]
    pub account: Account<TokenAccount>,
    pub mint: Account<Mint>,
    #[account(seeds = [TOKEN_CPI_EXT_AUTHORITY_SEED], bump)]
    pub authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct ReadMint {
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct ReadTokenAccount {
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckMintDecimals {
    #[account(mut, mint::decimals = 6)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct CheckMintAuthority {
    pub expected: UncheckedAccount,
    #[account(mut, mint::authority = expected)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct CheckMintFreezeAuthority {
    pub expected: UncheckedAccount,
    #[account(mut, mint::freeze_authority = expected)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct CheckMintTokenProgram {
    pub token_program: UncheckedAccount,
    #[account(mut, mint::token_program = token_program)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct CheckTokenMint {
    pub mint: Account<Mint>,
    #[account(mut, token::mint = mint)]
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckTokenAuthority {
    pub expected: UncheckedAccount,
    #[account(mut, token::authority = expected)]
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckTokenProgram {
    pub token_program: UncheckedAccount,
    #[account(mut, token::token_program = token_program)]
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckAta {
    pub authority: UncheckedAccount,
    pub mint: Account<Mint>,
    pub vault: Account<TokenAccount>,
}

// -- InterfaceAccount read-path structs --------------------------------------

#[derive(Accounts)]
pub struct ReadInterfaceMint {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
pub struct ReadInterfaceTokenAccount {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

// -- InterfaceAccount init-path structs --------------------------------------

#[derive(Accounts)]
pub struct InitInterfaceMint {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    pub token_program: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitInterfaceTokenAccount {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub authority: UncheckedAccount,
    pub token_program: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
    pub system_program: Program<System>,
}

// -- Namespaced-constraint structs on InterfaceAccount -----------------------

#[derive(Accounts)]
pub struct CheckInterfaceTokenMint {
    pub mint: InterfaceAccount<token_interface::Mint>,
    #[account(mut, token::mint = mint)]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckInterfaceTokenAuthority {
    pub expected: UncheckedAccount,
    #[account(mut, token::authority = expected)]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckInterfaceTokenProgram {
    pub token_program: UncheckedAccount,
    #[account(mut, token::token_program = token_program)]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintAuthority {
    pub expected: UncheckedAccount,
    #[account(mut, mint::authority = expected)]
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintFreezeAuthority {
    pub expected: UncheckedAccount,
    #[account(mut, mint::freeze_authority = expected)]
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintDecimals {
    #[account(mut, mint::decimals = 6)]
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintTokenProgram {
    pub token_program: UncheckedAccount,
    #[account(mut, mint::token_program = token_program)]
    pub mint: InterfaceAccount<token_interface::Mint>,
}

// -- Extension-reader structs ------------------------------------------------
//
// Each Token-2022 extension reader has its own Accounts struct so the
// struct's `#[instruction(...)]` list matches its handler's extra-args
// signature exactly. The single-deser contract (one Borsh parse of
// `ix_data`, shared between constraint validation and the handler) only
// works when each (accounts struct, handler) pair agrees on the wire
// shape — so handlers that previously shared `ReadMintExtension` /
// `ReadTokenAccountExtension` with divergent args get their own struct.

#[derive(Accounts)]
#[instruction(
    expected_withheld: u64,
    expected_older_epoch: u64,
    expected_older_max_fee: u64,
    expected_older_bps: u16,
    expected_newer_epoch: u64,
    expected_newer_max_fee: u64,
    expected_newer_bps: u16
)]
pub struct ReadTransferFeeConfig {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_bps: u16)]
pub struct ReadTransferFeeConfigViaTrait {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_authority: Address, expected_metadata: Address)]
pub struct ReadMetadataPointer {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_program_id: Address)]
pub struct ReadTransferHook {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_authority: Address, expect_none: u8)]
pub struct ReadMintCloseAuthority {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_delegate: Address, expect_none: u8)]
pub struct ReadPermanentDelegate {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_withheld: u64)]
pub struct ReadTransferFeeAmount {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
#[instruction(expected_withheld: u64)]
pub struct ReadTransferFeeAmountViaTrait {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
#[instruction(expected_transferring: u8)]
pub struct ReadTransferHookAccount {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
#[instruction(expected_state: u8)]
pub struct ReadDefaultAccountState {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_authority: Address, expected_group: Address)]
pub struct ReadGroupPointer {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_authority: Address, expected_member: Address)]
pub struct ReadGroupMemberPointer {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_enabled: u8)]
pub struct ReadCpiGuard {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
#[instruction(expected_authority: Address, expected_paused: u8)]
pub struct ReadPausableConfig {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
pub struct ReadMarkerExtensions {
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
#[instruction(expected_bps: u16)]
pub struct ReadUncheckedMintExtension {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_withheld: u64)]
pub struct ReadUncheckedTokenAccountExtension {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
pub struct ReadUncheckedMintTransferFeeAmount {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
pub struct ReadUncheckedTokenAccountTransferFeeConfig {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
}

#[derive(Accounts)]
pub struct Token2022GroupPointerUpdate {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022GroupMemberPointerUpdate {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022Reallocate {
    #[account(mut)]
    pub account: UncheckedAccount,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022ImmutableOwnerInitialize {
    #[account(mut)]
    pub token_account: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022NonTransferableMintInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022MintCloseAuthorityInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022PermanentDelegateInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022ReturnDataMint {
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022DefaultAccountStateInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022MemoTransfer {
    #[account(mut)]
    pub account: UncheckedAccount,
    pub owner: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022MetadataPointerInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TransferHookInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022InterestBearingMintInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022PausableInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TransferFeeInitialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022CreateNativeMint {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub native_mint: UncheckedAccount,
    pub system_program: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022WithdrawExcessLamports {
    #[account(mut)]
    pub source: UncheckedAccount,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TokenMetadataRemoveKey {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TokenMetadataInitialize {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: UncheckedAccount,
    pub mint_authority: Signer,
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TokenMetadataUpdateAuthority {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub current_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TokenMetadataUpdateField {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TokenGroupInitialize {
    #[account(mut)]
    pub group: UncheckedAccount,
    pub mint: UncheckedAccount,
    pub mint_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Token2022TokenMemberInitialize {
    #[account(mut)]
    pub member: UncheckedAccount,
    pub member_mint: UncheckedAccount,
    pub member_mint_authority: Signer,
    #[account(mut)]
    pub group: UncheckedAccount,
    pub group_update_authority: Signer,
    pub token_program: UncheckedAccount,
}
