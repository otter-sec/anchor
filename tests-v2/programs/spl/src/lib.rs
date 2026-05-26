//! Test program exercising `anchor-spl-v2`'s Mint/TokenAccount surface.
//!
//! Each handler targets a specific area of the SPL module — init codegen,
//! CPI helpers, accessor methods, namespaced constraints — so the
//! integration tests in `tests/spl.rs` can trip each path from a known
//! state and coverage attributes the execution back to the right file.

use {
    anchor_lang_v2::prelude::*,
    anchor_spl_v2::{
        associated_token::get_associated_token_address,
        extensions::{
            self, CpiGuard, DefaultAccountState, GroupMemberPointer, GroupPointer, MetadataPointer,
            MintCloseAuthority, NonTransferable, NonTransferableAccount, PausableAccount,
            PausableConfig, PermanentDelegate, TransferFeeAmount, TransferFeeConfig, TransferHook,
            TransferHookAccount,
        },
        mint::{self, Mint},
        token::{self, TokenAccount},
        token_2022 as token_2022_cpi, token_2022_extensions as token_2022_ext_cpi, token_interface,
    },
};

declare_id!("SpL1111111111111111111111111111111111111111");

#[program]
pub mod spl_test {
    use super::*;

    /// Create a new Mint account. Hits `mint::SlabInit::create_and_initialize`
    /// → `pinocchio_token::InitializeMint2`.
    #[discrim = 0]
    pub fn init_mint(_ctx: &mut Context<InitMint>) -> Result<()> {
        Ok(())
    }

    /// Create a new TokenAccount. Hits `token::SlabInit::create_and_initialize`
    /// → `pinocchio_token::InitializeAccount3`.
    #[discrim = 1]
    pub fn init_token_account(_ctx: &mut Context<InitTokenAccount>) -> Result<()> {
        Ok(())
    }

    /// Mint `amount` tokens into `to`. Hits `token::mint_to`.
    #[discrim = 2]
    pub fn do_mint_to(ctx: &mut Context<DoMintTo>, amount: u64) -> Result<()> {
        let accs = token::MintTo {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            to: ctx.accounts.to.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::mint_to(cpi_ctx, amount)?;
        Ok(())
    }

    /// Transfer `amount` tokens from `from` to `to`. Hits `token::transfer`.
    #[discrim = 3]
    pub fn do_transfer(ctx: &mut Context<DoTransfer>, amount: u64) -> Result<()> {
        let accs = token::Transfer {
            from: ctx.accounts.from.cpi_handle_mut(),
            to: ctx.accounts.to.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::transfer(cpi_ctx, amount)?;
        Ok(())
    }

    /// TransferChecked (also verifies decimals match mint). Hits
    /// `token::transfer_checked`.
    #[discrim = 4]
    pub fn do_transfer_checked(
        ctx: &mut Context<DoTransferChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let accs = token::TransferChecked {
            from: ctx.accounts.from.cpi_handle_mut(),
            mint: ctx.accounts.mint.cpi_handle(),
            to: ctx.accounts.to.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }

    /// Burn `amount` tokens from `account`. Hits `token::burn`.
    #[discrim = 5]
    pub fn do_burn(ctx: &mut Context<DoBurn>, amount: u64) -> Result<()> {
        let accs = token::Burn {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            from: ctx.accounts.account.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::burn(cpi_ctx, amount)?;
        Ok(())
    }

    /// Approve `delegate` to spend `amount` from `source`. Hits
    /// `token::approve`.
    #[discrim = 6]
    pub fn do_approve(ctx: &mut Context<DoApprove>, amount: u64) -> Result<()> {
        let accs = token::Approve {
            to: ctx.accounts.source.cpi_handle_mut(),
            delegate: ctx.accounts.delegate.cpi_handle(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::approve(cpi_ctx, amount)?;
        Ok(())
    }

    /// Revoke delegation. Hits `token::revoke`.
    #[discrim = 7]
    pub fn do_revoke(ctx: &mut Context<DoRevoke>) -> Result<()> {
        let accs = token::Revoke {
            source: ctx.accounts.source.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::revoke(cpi_ctx)?;
        Ok(())
    }

    /// Close `account`, reclaiming lamports to `destination`. Hits
    /// `token::close_account`.
    #[discrim = 8]
    pub fn do_close_account(ctx: &mut Context<DoCloseAccount>) -> Result<()> {
        let accs = token::CloseAccount {
            account: ctx.accounts.account.cpi_handle_mut(),
            destination: ctx.accounts.destination.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token::close_account(cpi_ctx)?;

        Ok(())
    }

    /// Reads every `Mint` accessor — supply, decimals, authority flags,
    /// freeze flags. Logs nothing (logging costs CUs); the assertion is that
    /// the call succeeds and the traces cover the accessor methods.
    #[discrim = 9]
    pub fn read_mint(ctx: &mut Context<ReadMint>) -> Result<()> {
        let m = &*ctx.accounts.mint;
        let _ = m.supply();
        let _ = m.decimals();
        let _ = m.has_mint_authority();
        let _ = m.mint_authority();
        let _ = m.is_initialized();
        let _ = m.has_freeze_authority();
        let _ = m.freeze_authority();
        Ok(())
    }

    /// Reads every `TokenAccount` accessor — amount, delegate flags, state,
    /// native/close flags. See `read_mint` for rationale.
    #[discrim = 10]
    pub fn read_token_account(ctx: &mut Context<ReadTokenAccount>) -> Result<()> {
        let ta = &*ctx.accounts.token_account;
        let _ = ta.mint();
        let _ = ta.owner();
        let _ = ta.amount();
        let _ = ta.delegated_amount();
        let _ = ta.has_delegate();
        let _ = ta.delegate();
        let _ = ta.state();
        let _ = ta.is_native();
        let _ = ta.native_amount();
        let _ = ta.has_close_authority();
        let _ = ta.close_authority();
        let _ = ta.is_initialized();
        let _ = ta.is_frozen();
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

    /// Verifies that `vault` is the canonical ATA for `(authority, mint)`.
    /// Exercises `get_associated_token_address`.
    #[discrim = 15]
    pub fn check_ata(ctx: &mut Context<CheckAta>) -> Result<()> {
        let expected = get_associated_token_address(
            ctx.accounts.authority.account().address(),
            ctx.accounts.mint.account().address(),
        );
        if *ctx.accounts.vault.account().address() != expected {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    // ---- InterfaceAccount read path ---------------------------------------

    /// Load an `InterfaceAccount<Mint>` and touch accessors. Succeeds when the
    /// underlying account is owned by either Token or Token-2022.
    #[discrim = 16]
    pub fn read_interface_mint(ctx: &mut Context<ReadInterfaceMint>) -> Result<()> {
        let m = &*ctx.accounts.mint;
        let _ = m.supply();
        let _ = m.decimals();
        let _ = m.mint_authority();
        let _ = m.freeze_authority();
        let _ = m.is_initialized();
        Ok(())
    }

    /// Load an `InterfaceAccount<TokenAccount>` and touch accessors.
    #[discrim = 17]
    pub fn read_interface_token_account(
        ctx: &mut Context<ReadInterfaceTokenAccount>,
    ) -> Result<()> {
        let ta = &*ctx.accounts.token_account;
        let _ = ta.mint();
        let _ = ta.owner();
        let _ = ta.amount();
        Ok(())
    }

    // ---- InterfaceAccount init path ---------------------------------------

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
        expected_bps: u16,
    ) -> Result<()> {
        let ext: &TransferFeeConfig = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.newer_transfer_fee.basis_points() != expected_bps {
            return Err(ProgramError::InvalidAccountData.into());
        }
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
        let ext: &MetadataPointer = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.authority != expected_authority || ext.metadata_address != expected_metadata {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `TransferHook` and assert the `program_id` matches.
    #[discrim = 29]
    pub fn read_transfer_hook(
        ctx: &mut Context<ReadTransferHook>,
        expected_program_id: Address,
    ) -> Result<()> {
        let ext: &TransferHook = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.program_id != expected_program_id {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `MintCloseAuthority`, exercising `optional_address` on the
    /// resulting field — returns an error when the close authority is unset.
    #[discrim = 30]
    pub fn read_mint_close_authority(
        ctx: &mut Context<ReadMintCloseAuthority>,
        expected_authority: Address,
    ) -> Result<()> {
        let ext: &MintCloseAuthority = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        match extensions::optional_address(&ext.close_authority) {
            Some(addr) if *addr == expected_authority => Ok(()),
            _ => Err(ProgramError::InvalidAccountData.into()),
        }
    }

    /// Parse `PermanentDelegate` and compare via `optional_address`.
    #[discrim = 31]
    pub fn read_permanent_delegate(
        ctx: &mut Context<ReadPermanentDelegate>,
        expected_delegate: Address,
    ) -> Result<()> {
        let ext: &PermanentDelegate = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        match extensions::optional_address(&ext.delegate) {
            Some(addr) if *addr == expected_delegate => Ok(()),
            _ => Err(ProgramError::InvalidAccountData.into()),
        }
    }

    /// Parse `TransferFeeAmount` from a Token-2022 token account.
    #[discrim = 32]
    pub fn read_transfer_fee_amount(
        ctx: &mut Context<ReadTransferFeeAmount>,
        expected_withheld: u64,
    ) -> Result<()> {
        let ext: &TransferFeeAmount =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        if ext.withheld_amount() != expected_withheld {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `TransferHookAccount` from a Token-2022 token account.
    #[discrim = 33]
    pub fn read_transfer_hook_account(
        ctx: &mut Context<ReadTransferHookAccount>,
        expected_transferring: u8,
    ) -> Result<()> {
        let ext: &TransferHookAccount =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        if ext.transferring != expected_transferring {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `DefaultAccountState` from a Token-2022 mint.
    #[discrim = 34]
    pub fn read_default_account_state(
        ctx: &mut Context<ReadDefaultAccountState>,
        expected_state: u8,
    ) -> Result<()> {
        let ext: &DefaultAccountState =
            extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.state != expected_state {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `GroupPointer` and assert both optional addresses match.
    #[discrim = 35]
    pub fn read_group_pointer(
        ctx: &mut Context<ReadGroupPointer>,
        expected_authority: Address,
        expected_group: Address,
    ) -> Result<()> {
        let ext: &GroupPointer = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.authority != expected_authority || ext.group_address != expected_group {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `GroupMemberPointer` and assert both optional addresses match.
    #[discrim = 36]
    pub fn read_group_member_pointer(
        ctx: &mut Context<ReadGroupMemberPointer>,
        expected_authority: Address,
        expected_member: Address,
    ) -> Result<()> {
        let ext: &GroupMemberPointer = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.authority != expected_authority || ext.member_address != expected_member {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `CpiGuard` from a Token-2022 token account.
    #[discrim = 37]
    pub fn read_cpi_guard(ctx: &mut Context<ReadCpiGuard>, expected_enabled: u8) -> Result<()> {
        let ext: &CpiGuard =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        if u8::from(ext.is_enabled()) != expected_enabled {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse `PausableConfig` from a Token-2022 mint.
    #[discrim = 38]
    pub fn read_pausable_config(
        ctx: &mut Context<ReadPausableConfig>,
        expected_authority: Address,
        expected_paused: u8,
    ) -> Result<()> {
        let ext: &PausableConfig = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.authority != expected_authority || u8::from(ext.is_paused()) != expected_paused {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse zero-sized mint/account marker extensions.
    #[discrim = 39]
    pub fn read_marker_extensions(ctx: &mut Context<ReadMarkerExtensions>) -> Result<()> {
        let _: &NonTransferable = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        let _: &NonTransferableAccount =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        let _: &PausableAccount =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        Ok(())
    }

    /// Parse a mint extension from an unchecked account. Used to assert that
    /// `extensions::get_mint_extension` performs its own validation.
    #[discrim = 45]
    pub fn read_unchecked_transfer_fee_config(
        ctx: &mut Context<ReadUncheckedMintExtension>,
        expected_bps: u16,
    ) -> Result<()> {
        let ext: &TransferFeeConfig = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        if ext.newer_transfer_fee.basis_points() != expected_bps {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Parse a token account extension from an unchecked account. Used to
    /// assert that `extensions::get_token_account_extension` validates base
    /// Token-2022 account shape before walking TLV.
    #[discrim = 46]
    pub fn read_unchecked_transfer_fee_amount(
        ctx: &mut Context<ReadUncheckedTokenAccountExtension>,
        expected_withheld: u64,
    ) -> Result<()> {
        let ext: &TransferFeeAmount =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        if ext.withheld_amount() != expected_withheld {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    /// Attempt to parse an account extension from a mint-shaped unchecked
    /// account. This must fail through SPL Token-2022's extension-family check.
    #[discrim = 53]
    pub fn read_unchecked_mint_transfer_fee_amount(
        ctx: &mut Context<ReadUncheckedMintTransferFeeAmount>,
    ) -> Result<()> {
        let _: &TransferFeeAmount = extensions::get_mint_extension(ctx.accounts.mint.account())?;
        Ok(())
    }

    /// Attempt to parse a mint extension from a token-account-shaped unchecked
    /// account. This must fail through SPL Token-2022's extension-family check.
    #[discrim = 54]
    pub fn read_unchecked_token_account_transfer_fee_config(
        ctx: &mut Context<ReadUncheckedTokenAccountTransferFeeConfig>,
    ) -> Result<()> {
        let _: &TransferFeeConfig =
            extensions::get_token_account_extension(ctx.accounts.token_account.account())?;
        Ok(())
    }

    /// Invoke the Token-2022 CPI Guard helper against the spy program.
    #[discrim = 40]
    pub fn spy_cpi_guard_enable(ctx: &mut Context<SpyCpiGuard>) -> Result<()> {
        let accs = token_2022_ext_cpi::CpiGuard {
            account: ctx.accounts.account.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        #[allow(deprecated)]
        token_2022_ext_cpi::cpi_guard_enable(cpi_ctx)?;
        Ok(())
    }

    /// Invoke the Token-2022 group pointer update helper against the spy program.
    #[discrim = 41]
    pub fn spy_group_pointer_update(
        ctx: &mut Context<SpyGroupPointerUpdate>,
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

    /// Invoke the Token-2022 group member pointer update helper against the spy program.
    #[discrim = 42]
    pub fn spy_group_member_pointer_update(
        ctx: &mut Context<SpyGroupMemberPointerUpdate>,
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

    /// Invoke the Token-2022 reallocate helper against the spy program.
    #[discrim = 43]
    pub fn spy_reallocate_group_pointer(ctx: &mut Context<SpyReallocate>) -> Result<()> {
        let accs = token_2022_cpi::Reallocate {
            account: ctx.accounts.account.cpi_handle_mut(),
            payer: ctx.accounts.payer.cpi_handle_mut(),
            system_program: ctx.accounts.system_program.cpi_handle(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_cpi::reallocate(cpi_ctx, &[token_2022_cpi::ExtensionType::GroupPointer])?;

        Ok(())
    }

    /// Invoke Token Metadata remove_key against the spy program.
    #[discrim = 47]
    pub fn spy_token_metadata_remove_key(
        ctx: &mut Context<SpyTokenMetadataRemoveKey>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMetadataRemoveKey {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_metadata_remove_key(cpi_ctx, "field".into(), true)?;
        Ok(())
    }

    /// Invoke Token Metadata initialize against the spy program.
    #[discrim = 48]
    pub fn spy_token_metadata_initialize(
        ctx: &mut Context<SpyTokenMetadataInitialize>,
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

    /// Invoke Token Metadata update_authority against the spy program.
    #[discrim = 49]
    pub fn spy_token_metadata_update_authority(
        ctx: &mut Context<SpyTokenMetadataUpdateAuthority>,
    ) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenMetadataUpdateAuthority {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            current_authority: ctx.accounts.current_authority.cpi_handle(),
            new_authority: ctx.accounts.new_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_metadata_update_authority(
            cpi_ctx,
            Some(ctx.accounts.new_authority.address()),
        )?;
        Ok(())
    }

    /// Invoke Token Metadata update_field against the spy program.
    #[discrim = 50]
    pub fn spy_token_metadata_update_field(
        ctx: &mut Context<SpyTokenMetadataUpdateField>,
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

    /// Invoke Token Group initialize against the spy program.
    #[discrim = 51]
    pub fn spy_token_group_initialize(ctx: &mut Context<SpyTokenGroupInitialize>) -> Result<()> {
        let accs = token_2022_ext_cpi::TokenGroupInitialize {
            group: ctx.accounts.group.cpi_handle_mut(),
            mint: ctx.accounts.mint.cpi_handle(),
            mint_authority: ctx.accounts.mint_authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.address(), accs);
        token_2022_ext_cpi::token_group_initialize(cpi_ctx, None, 10)?;
        Ok(())
    }

    /// Invoke Token Group member initialize against the spy program.
    #[discrim = 52]
    pub fn spy_token_member_initialize(ctx: &mut Context<SpyTokenMemberInitialize>) -> Result<()> {
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
#[instruction(expected_bps: u16)]
pub struct ReadTransferFeeConfig {
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
#[instruction(expected_authority: Address)]
pub struct ReadMintCloseAuthority {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_delegate: Address)]
pub struct ReadPermanentDelegate {
    pub mint: InterfaceAccount<token_interface::Mint>,
}

#[derive(Accounts)]
#[instruction(expected_withheld: u64)]
pub struct ReadTransferFeeAmount {
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
    pub mint: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(expected_withheld: u64)]
pub struct ReadUncheckedTokenAccountExtension {
    pub token_account: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ReadUncheckedMintTransferFeeAmount {
    pub mint: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ReadUncheckedTokenAccountTransferFeeConfig {
    pub token_account: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyCpiGuard {
    #[account(mut)]
    pub account: UncheckedAccount,
    pub owner: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyGroupPointerUpdate {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyGroupMemberPointerUpdate {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyReallocate {
    #[account(mut)]
    pub account: UncheckedAccount,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyTokenMetadataRemoveKey {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyTokenMetadataInitialize {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: UncheckedAccount,
    pub mint_authority: Signer,
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyTokenMetadataUpdateAuthority {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub current_authority: Signer,
    pub new_authority: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyTokenMetadataUpdateField {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyTokenGroupInitialize {
    #[account(mut)]
    pub group: UncheckedAccount,
    pub mint: UncheckedAccount,
    pub mint_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SpyTokenMemberInitialize {
    #[account(mut)]
    pub member: UncheckedAccount,
    pub member_mint: UncheckedAccount,
    pub member_mint_authority: Signer,
    #[account(mut)]
    pub group: UncheckedAccount,
    pub group_update_authority: Signer,
    pub token_program: UncheckedAccount,
}
