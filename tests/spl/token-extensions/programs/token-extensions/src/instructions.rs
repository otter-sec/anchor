use {
    crate::{
        get_meta_list_size, get_mint_extensible_extension_data,
        update_account_lamports_to_minimum_balance, META_LIST_ACCOUNT_SEED,
    },
    anchor_lang::{prelude::*, solana_program::entrypoint::ProgramResult},
    anchor_spl::{
        associated_token::AssociatedToken,
        token_2022::{
            self,
            spl_token_2022::extension::{
                group_member_pointer::GroupMemberPointer,
                interest_bearing_mint::{BasisPoints, InterestBearingConfig},
                metadata_pointer::MetadataPointer,
                mint_close_authority::MintCloseAuthority,
                pausable::PausableConfig,
                permanent_delegate::PermanentDelegate,
                transfer_hook::TransferHook,
            },
        },
        token_2022_extensions,
        token_interface::{
            get_mint_extension_data, pausable_pause, pausable_resume,
            spl_token_metadata_interface::state::{Field, TokenMetadata},
            token_metadata_initialize, token_metadata_remove_key, token_metadata_update_field,
            Mint, PausableToggle, Token2022, TokenAccount, TokenMetadataInitialize,
            TokenMetadataRemoveKey, TokenMetadataUpdateField,
        },
    },
    spl_pod::{optional_keys::OptionalNonZeroPubkey, primitives::PodBool},
};

#[derive(AnchorDeserialize, AnchorSerialize)]
pub struct CreateMintAccountArgs {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

#[derive(Accounts)]
#[instruction(args: CreateMintAccountArgs)]
pub struct CreateMintAccount<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    /// CHECK: can be any account
    pub authority: Signer<'info>,
    #[account()]
    /// CHECK: can be any account
    pub receiver: UncheckedAccount<'info>,
    #[account(
        init,
        signer,
        payer = payer,
        mint::token_program = token_program,
        mint::decimals = 0,
        mint::authority = authority,
        mint::freeze_authority = authority,
        extensions::metadata_pointer::authority = authority,
        extensions::metadata_pointer::metadata_address = mint,
        extensions::group_member_pointer::authority = authority,
        extensions::group_member_pointer::member_address = mint,
        extensions::transfer_hook::authority = authority,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::close_authority::authority = authority,
        extensions::permanent_delegate::delegate = authority,
        extensions::pausable::authority = authority,
        extensions::interest_bearing_mint::authority = authority,
        extensions::interest_bearing_mint::rate = 100,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        init,
        payer = payer,
        associated_token::token_program = token_program,
        associated_token::mint = mint,
        associated_token::authority = receiver,
    )]
    pub mint_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    /// CHECK: This account's data is a buffer of TLV data
    #[account(
        init,
        space = get_meta_list_size(None),
        seeds = [META_LIST_ACCOUNT_SEED, mint.key().as_ref()],
        bump,
        payer = payer,
    )]
    pub extra_metas_account: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token2022>,
}

impl<'info> CreateMintAccount<'info> {
    fn initialize_token_metadata(
        &self,
        name: String,
        symbol: String,
        uri: String,
    ) -> ProgramResult {
        let cpi_accounts = TokenMetadataInitialize {
            program_id: self.token_program.to_account_info(),
            mint: self.mint.to_account_info(),
            metadata: self.mint.to_account_info(), // metadata account is the mint, since data is stored in mint
            mint_authority: self.authority.to_account_info(),
            update_authority: self.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.token_program.key(), cpi_accounts);
        token_metadata_initialize(cpi_ctx, name, symbol, uri)?;
        Ok(())
    }
}

pub fn handler(ctx: Context<CreateMintAccount>, args: CreateMintAccountArgs) -> Result<()> {
    ctx.accounts.initialize_token_metadata(
        args.name.clone(),
        args.symbol.clone(),
        args.uri.clone(),
    )?;
    ctx.accounts.mint.reload()?;
    let mint_data = &mut ctx.accounts.mint.to_account_info();
    let metadata = get_mint_extensible_extension_data::<TokenMetadata>(mint_data)?;
    assert_eq!(metadata.mint, ctx.accounts.mint.key());
    assert_eq!(metadata.name, args.name);
    assert_eq!(metadata.symbol, args.symbol);
    assert_eq!(metadata.uri, args.uri);
    let metadata_pointer = get_mint_extension_data::<MetadataPointer>(mint_data)?;
    let mint_key: Option<Pubkey> = Some(ctx.accounts.mint.key());
    let authority_key: Option<Pubkey> = Some(ctx.accounts.authority.key());
    assert_eq!(
        metadata_pointer.metadata_address,
        OptionalNonZeroPubkey::try_from(mint_key)?
    );
    assert_eq!(
        metadata_pointer.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    let permanent_delegate = get_mint_extension_data::<PermanentDelegate>(mint_data)?;
    assert_eq!(
        permanent_delegate.delegate,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    let close_authority = get_mint_extension_data::<MintCloseAuthority>(mint_data)?;
    assert_eq!(
        close_authority.close_authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    let transfer_hook = get_mint_extension_data::<TransferHook>(mint_data)?;
    let program_id: Option<Pubkey> = Some(ctx.program_id.key());
    assert_eq!(
        transfer_hook.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    assert_eq!(
        transfer_hook.program_id,
        OptionalNonZeroPubkey::try_from(program_id)?
    );
    let group_member_pointer = get_mint_extension_data::<GroupMemberPointer>(mint_data)?;
    assert_eq!(
        group_member_pointer.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    assert_eq!(
        group_member_pointer.member_address,
        OptionalNonZeroPubkey::try_from(mint_key)?
    );
    let interest_bearing_config = get_mint_extension_data::<InterestBearingConfig>(mint_data)?;
    assert_eq!(
        interest_bearing_config.rate_authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    assert_eq!(interest_bearing_config.current_rate, BasisPoints::from(100));
    // transfer minimum rent to mint account
    update_account_lamports_to_minimum_balance(
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
    )?;

    let pausable_extension = get_mint_extension_data::<PausableConfig>(mint_data)?;
    assert_eq!(
        pausable_extension.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction()]
pub struct CheckMintExtensionConstraints<'info> {
    #[account(mut)]
    /// CHECK: can be any account
    pub authority: Signer<'info>,
    #[account(
        extensions::metadata_pointer::authority = authority,
        extensions::metadata_pointer::metadata_address = mint,
        extensions::group_member_pointer::authority = authority,
        extensions::group_member_pointer::member_address = mint,
        extensions::transfer_hook::authority = authority,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::close_authority::authority = authority,
        extensions::permanent_delegate::delegate = authority,
        extensions::pausable::authority = authority,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct CreateGroupPointerMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(
        init,
        signer,
        payer = payer,
        mint::token_program = token_program,
        mint::decimals = 0,
        mint::authority = authority,
        extensions::group_pointer::authority = authority,
        extensions::group_pointer::group_address = mint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct UpdateGroupPointer<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    pub token_program: Program<'info, Token2022>,
}

pub fn update_group_pointer_handler(
    ctx: Context<UpdateGroupPointer>,
    new_group_address: Option<Pubkey>,
) -> Result<()> {
    let cpi_accounts = token_2022_extensions::group_pointer::GroupPointerUpdate {
        token_program_id: ctx.accounts.token_program.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(*ctx.accounts.token_program.key, cpi_accounts);
    token_2022_extensions::group_pointer::group_pointer_update(cpi_ctx, new_group_address)
}

#[derive(Accounts)]
#[instruction()]
pub struct CheckTogglePause<'info> {
    #[account(mut)]
    /// CHECK: can be any account
    pub authority: Signer<'info>,
    #[account(
        mut,
        extensions::pausable::authority = authority,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Program<'info, Token2022>,
}

pub fn toggle_pause_handler(ctx: Context<CheckTogglePause>) -> Result<()> {
    let mint_data = &mut ctx.accounts.mint.to_account_info();
    let pausable_extension = get_mint_extension_data::<PausableConfig>(mint_data)?;
    assert_eq!(pausable_extension.paused, PodBool::from_bool(false));

    pausable_pause(CpiContext::new(
        Token2022::id(),
        PausableToggle {
            token_program_id: ctx.accounts.token_program.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        },
    ))?;

    let pausable_extension = get_mint_extension_data::<PausableConfig>(mint_data)?;
    assert_eq!(pausable_extension.paused, PodBool::from_bool(true));

    pausable_resume(CpiContext::new(
        Token2022::id(),
        PausableToggle {
            token_program_id: ctx.accounts.token_program.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        },
    ))?;

    let pausable_extension = get_mint_extension_data::<PausableConfig>(mint_data)?;
    assert_eq!(pausable_extension.paused, PodBool::from_bool(false));

    Ok(())
}

#[derive(Accounts)]
pub struct CheckPausableAuthorityConstraint<'info> {
    pub authority: Signer<'info>,
    #[account(extensions::pausable::authority = authority)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
}

pub fn check_pausable_authority_constraint_handler(
    _ctx: Context<CheckPausableAuthorityConstraint>,
) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateAndRemoveTokenMetadata<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Program<'info, Token2022>,
}

impl<'info> UpdateAndRemoveTokenMetadata<'info> {
    fn update_token_metadata(&self, field: String, value: String) -> ProgramResult {
        let cpi_accounts = TokenMetadataUpdateField {
            program_id: self.token_program.to_account_info(),
            metadata: self.mint.to_account_info(), // metadata account is the mint, since data is stored in mint
            update_authority: self.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.token_program.key(), cpi_accounts);
        token_metadata_update_field(cpi_ctx, Field::Key(field), value)?;
        Ok(())
    }

    fn remove_token_metadata(&self, key: String) -> ProgramResult {
        let cpi_accounts = TokenMetadataRemoveKey {
            program_id: self.token_program.to_account_info(),
            metadata: self.mint.to_account_info(), // metadata account is the mint, since data is stored in mint
            update_authority: self.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.token_program.key(), cpi_accounts);
        token_metadata_remove_key(cpi_ctx, key, true)?;
        Ok(())
    }
}

pub fn update_and_remove_token_metadata_handler(
    ctx: Context<UpdateAndRemoveTokenMetadata>,
) -> Result<()> {
    let key = "dummy_key";
    let value = "dummy_value";
    ctx.accounts
        .update_token_metadata(key.to_string(), value.to_string())?;
    ctx.accounts.mint.reload()?;

    let mint_data = &mut ctx.accounts.mint.to_account_info();
    let metadata = get_mint_extensible_extension_data::<TokenMetadata>(mint_data)?;
    assert_eq!(metadata.additional_metadata.len(), 1);

    if let Some((k, v)) = metadata.additional_metadata.get(0) {
        assert_eq!(k, key);
        assert_eq!(v, value);
    } else {
        assert!(false);
    }

    ctx.accounts.remove_token_metadata(key.to_string())?;
    ctx.accounts.mint.reload()?;

    let mint_data = &mut ctx.accounts.mint.to_account_info();
    let metadata = get_mint_extensible_extension_data::<TokenMetadata>(mint_data)?;
    assert_eq!(metadata.additional_metadata.len(), 0);
    Ok(())
}

#[derive(Accounts)]
pub struct CpiCreateNativeMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Native mint address; the Token-2022 program verifies the PDA.
    #[account(mut)]
    pub native_mint: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
}

pub fn cpi_create_native_mint_handler(ctx: Context<CpiCreateNativeMint>) -> Result<()> {
    let cpi_accounts = token_2022::CreateNativeMint {
        payer: ctx.accounts.payer.to_account_info(),
        native_mint: ctx.accounts.native_mint.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(*ctx.accounts.token_program.key, cpi_accounts);
    token_2022::create_native_mint(cpi_ctx)
}

#[derive(Accounts)]
pub struct CpiInitializeNonTransferableMint<'info> {
    /// CHECK: Uninitialized mint account allocated by the client before this ix.
    #[account(mut)]
    pub mint: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token2022>,
}

pub fn cpi_initialize_non_transferable_mint_handler(
    ctx: Context<CpiInitializeNonTransferableMint>,
) -> Result<()> {
    let cpi_accounts = token_2022::InitializeNonTransferableMint {
        mint: ctx.accounts.mint.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(*ctx.accounts.token_program.key, cpi_accounts);
    token_2022::initialize_non_transferable_mint(cpi_ctx)
}
