//! Metaplex Token Metadata CPI helpers and account wrappers.
//!
//! This mirrors Anchor v1's `anchor_spl::metadata` surface while using the v2
//! CPI/account primitives. Metaplex accounts are external Borsh accounts and do
//! not carry Anchor discriminators, so the account wrappers below deserialize
//! the raw Metaplex bytes directly.

extern crate alloc;

pub use mpl_token_metadata;

use {
    alloc::{vec, vec::Vec},
    anchor_lang_v2::{
        AccountDeserialize, AnchorAccount, CpiContext, CpiHandle, Id, IdlAccountType, Result,
        ToCpiAccounts,
    },
    core::ops::Deref,
    pinocchio::{account::AccountView, instruction::InstructionAccount},
    solana_address::Address,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

pub const ID: Address = Address::from_str_const("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

macro_rules! impl_cpi_accounts {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl<'a> ToCpiAccounts<'a> for $name<'a> {
            fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
                vec![$(
                    InstructionAccount::new(
                        self.$field.address(),
                        self.$field.is_writable(),
                        self.$field.is_signer(),
                    ),
                )*]
            }

            fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
                vec![$(self.$field),*]
            }
        }
    };
}
pub fn approve_collection_authority<'info>(
    ctx: CpiContext<'info, ApproveCollectionAuthority<'info>>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::ApproveCollectionAuthority {
        collection_authority_record: *ctx.accounts.collection_authority_record.address(),
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        new_collection_authority: *ctx.accounts.new_collection_authority.address(),
        payer: *ctx.accounts.payer.address(),
        rent: None,
        system_program: anchor_lang_v2::programs::System::id(),
        update_authority: *ctx.accounts.update_authority.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn bubblegum_set_collection_size<'info>(
    ctx: CpiContext<'info, BubblegumSetCollectionSize<'info>>,
    collection_authority_record: Option<Pubkey>,
    size: u64,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::BubblegumSetCollectionSize {
        collection_metadata: *ctx.accounts.metadata_account.address(),
        collection_authority: *ctx.accounts.update_authority.address(),
        collection_mint: *ctx.accounts.mint.address(),
        bubblegum_signer: *ctx.accounts.bubblegum_signer.address(),
        collection_authority_record,
    }
    .instruction(
        mpl_token_metadata::instructions::BubblegumSetCollectionSizeInstructionArgs {
            set_collection_size_args: mpl_token_metadata::types::SetCollectionSizeArgs { size },
        },
    );
    ctx.invoke_ix(ix)
}

pub fn burn_edition_nft<'info>(ctx: CpiContext<'info, BurnEditionNft<'info>>) -> Result<()> {
    let ix = mpl_token_metadata::instructions::BurnEditionNft {
        edition_marker_account: *ctx.accounts.edition_marker.address(),
        master_edition_account: *ctx.accounts.master_edition.address(),
        master_edition_mint: *ctx.accounts.master_edition_mint.address(),
        master_edition_token_account: *ctx.accounts.master_edition_token.address(),
        metadata: *ctx.accounts.metadata.address(),
        owner: *ctx.accounts.owner.address(),
        print_edition_account: *ctx.accounts.print_edition.address(),
        print_edition_mint: *ctx.accounts.print_edition_mint.address(),
        print_edition_token_account: *ctx.accounts.print_edition_token.address(),
        spl_token_program: *ctx.accounts.spl_token.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

/// Burn an NFT by closing its token, metadata and edition accounts.
///
/// The lamports of the closed accounts will be transferred to the owner.
///
/// # Note
///
/// This instruction takes an optional `collection_metadata` argument, if this argument is
/// `Some`, the `ctx` argument should also include the `collection_metadata` account in its
/// remaining accounts, otherwise the CPI will fail because [`BurnNft`] only includes required
/// accounts.
///
/// ```ignore
/// CpiContext::new(program, BurnNft { .. })
///     .with_remaining_accounts(vec![ctx.accounts.collection_metadata]);
/// ```
pub fn burn_nft<'info>(
    ctx: CpiContext<'info, BurnNft<'info>>,
    collection_metadata: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::BurnNft {
        collection_metadata,
        master_edition_account: *ctx.accounts.edition.address(),
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        owner: *ctx.accounts.owner.address(),
        spl_token_program: *ctx.accounts.spl_token.address(),
        token_account: *ctx.accounts.token.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn create_metadata_accounts_v3<'info>(
    ctx: CpiContext<'info, CreateMetadataAccountsV3<'info>>,
    data: mpl_token_metadata::types::DataV2,
    is_mutable: bool,
    update_authority_is_signer: bool,
    collection_details: Option<mpl_token_metadata::types::CollectionDetails>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::CreateMetadataAccountV3 {
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        mint_authority: *ctx.accounts.mint_authority.address(),
        payer: *ctx.accounts.payer.address(),
        rent: None,
        system_program: anchor_lang_v2::programs::System::id(),
        update_authority: (
            *ctx.accounts.update_authority.address(),
            update_authority_is_signer,
        ),
    }
    .instruction(
        mpl_token_metadata::instructions::CreateMetadataAccountV3InstructionArgs {
            collection_details,
            data,
            is_mutable,
        },
    );
    ctx.invoke_ix(ix)
}

pub fn update_metadata_accounts_v2<'info>(
    ctx: CpiContext<'info, UpdateMetadataAccountsV2<'info>>,
    new_update_authority: Option<Pubkey>,
    data: Option<mpl_token_metadata::types::DataV2>,
    primary_sale_happened: Option<bool>,
    is_mutable: Option<bool>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::UpdateMetadataAccountV2 {
        metadata: *ctx.accounts.metadata.address(),
        update_authority: *ctx.accounts.update_authority.address(),
    }
    .instruction(
        mpl_token_metadata::instructions::UpdateMetadataAccountV2InstructionArgs {
            new_update_authority,
            data,
            primary_sale_happened,
            is_mutable,
        },
    );
    ctx.invoke_ix(ix)
}

pub fn create_master_edition_v3<'info>(
    ctx: CpiContext<'info, CreateMasterEditionV3<'info>>,
    max_supply: Option<u64>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::CreateMasterEditionV3 {
        edition: *ctx.accounts.edition.address(),
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        mint_authority: *ctx.accounts.mint_authority.address(),
        payer: *ctx.accounts.payer.address(),
        rent: None,
        system_program: anchor_lang_v2::programs::System::id(),
        token_program: spl_token_interface::ID,
        update_authority: *ctx.accounts.update_authority.address(),
    }
    .instruction(
        mpl_token_metadata::instructions::CreateMasterEditionV3InstructionArgs { max_supply },
    );
    ctx.invoke_ix(ix)
}

pub fn mint_new_edition_from_master_edition_via_token<'info>(
    ctx: CpiContext<'info, MintNewEditionFromMasterEditionViaToken<'info>>,
    edition: u64,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::MintNewEditionFromMasterEditionViaToken {
        edition_mark_pda: *ctx.accounts.edition_mark_pda.address(),
        master_edition: *ctx.accounts.master_edition.address(),
        metadata: *ctx.accounts.metadata.address(),
        new_edition: *ctx.accounts.new_edition.address(),
        new_metadata: *ctx.accounts.new_metadata.address(),
        new_metadata_update_authority: *ctx.accounts.new_metadata_update_authority.address(),
        new_mint: *ctx.accounts.new_mint.address(),
        new_mint_authority: *ctx.accounts.new_mint_authority.address(),
        payer: *ctx.accounts.payer.address(),
        rent: None,
        system_program: anchor_lang_v2::programs::System::id(),
        token_account: *ctx.accounts.token_account.address(),
        token_account_owner: *ctx.accounts.token_account_owner.address(),
        token_program: spl_token_interface::ID,
    }
    .instruction(
        mpl_token_metadata::instructions::MintNewEditionFromMasterEditionViaTokenInstructionArgs {
            mint_new_edition_from_master_edition_via_token_args:
                mpl_token_metadata::types::MintNewEditionFromMasterEditionViaTokenArgs { edition },
        },
    );
    ctx.invoke_ix(ix)
}

pub fn revoke_collection_authority<'info>(
    ctx: CpiContext<'info, RevokeCollectionAuthority<'info>>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::RevokeCollectionAuthority {
        collection_authority_record: *ctx.accounts.collection_authority_record.address(),
        delegate_authority: *ctx.accounts.delegate_authority.address(),
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        revoke_authority: *ctx.accounts.revoke_authority.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn set_collection_size<'info>(
    ctx: CpiContext<'info, SetCollectionSize<'info>>,
    collection_authority_record: Option<Pubkey>,
    size: u64,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::SetCollectionSize {
        collection_authority: *ctx.accounts.update_authority.address(),
        collection_authority_record,
        collection_metadata: *ctx.accounts.metadata.address(),
        collection_mint: *ctx.accounts.mint.address(),
    }
    .instruction(
        mpl_token_metadata::instructions::SetCollectionSizeInstructionArgs {
            set_collection_size_args: mpl_token_metadata::types::SetCollectionSizeArgs { size },
        },
    );
    ctx.invoke_ix(ix)
}

pub fn verify_collection<'info>(
    ctx: CpiContext<'info, VerifyCollection<'info>>,
    collection_authority_record: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::VerifyCollection {
        collection: *ctx.accounts.collection_metadata.address(),
        collection_authority: *ctx.accounts.collection_authority.address(),
        collection_authority_record,
        collection_master_edition_account: *ctx.accounts.collection_master_edition.address(),
        collection_mint: *ctx.accounts.collection_mint.address(),
        metadata: *ctx.accounts.metadata.address(),
        payer: *ctx.accounts.payer.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn verify_sized_collection_item<'info>(
    ctx: CpiContext<'info, VerifySizedCollectionItem<'info>>,
    collection_authority_record: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::VerifySizedCollectionItem {
        collection: *ctx.accounts.collection_metadata.address(),
        collection_authority: *ctx.accounts.collection_authority.address(),
        collection_authority_record,
        collection_master_edition_account: *ctx.accounts.collection_master_edition.address(),
        collection_mint: *ctx.accounts.collection_mint.address(),
        metadata: *ctx.accounts.metadata.address(),
        payer: *ctx.accounts.payer.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn set_and_verify_collection<'info>(
    ctx: CpiContext<'info, SetAndVerifyCollection<'info>>,
    collection_authority_record: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::SetAndVerifyCollection {
        collection: *ctx.accounts.collection_metadata.address(),
        collection_authority: *ctx.accounts.collection_authority.address(),
        collection_authority_record,
        collection_master_edition_account: *ctx.accounts.collection_master_edition.address(),
        collection_mint: *ctx.accounts.collection_mint.address(),
        metadata: *ctx.accounts.metadata.address(),
        payer: *ctx.accounts.payer.address(),
        update_authority: *ctx.accounts.update_authority.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn set_and_verify_sized_collection_item<'info>(
    ctx: CpiContext<'info, SetAndVerifySizedCollectionItem<'info>>,
    collection_authority_record: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::SetAndVerifySizedCollectionItem {
        collection: *ctx.accounts.collection_metadata.address(),
        collection_authority: *ctx.accounts.collection_authority.address(),
        collection_authority_record,
        collection_master_edition_account: *ctx.accounts.collection_master_edition.address(),
        collection_mint: *ctx.accounts.collection_mint.address(),
        metadata: *ctx.accounts.metadata.address(),
        payer: *ctx.accounts.payer.address(),
        update_authority: *ctx.accounts.update_authority.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn freeze_delegated_account<'info>(
    ctx: CpiContext<'info, FreezeDelegatedAccount<'info>>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::FreezeDelegatedAccount {
        delegate: *ctx.accounts.delegate.address(),
        edition: *ctx.accounts.edition.address(),
        mint: *ctx.accounts.mint.address(),
        token_account: *ctx.accounts.token_account.address(),
        token_program: *ctx.accounts.token_program.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn thaw_delegated_account<'info>(
    ctx: CpiContext<'info, ThawDelegatedAccount<'info>>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::ThawDelegatedAccount {
        delegate: *ctx.accounts.delegate.address(),
        edition: *ctx.accounts.edition.address(),
        mint: *ctx.accounts.mint.address(),
        token_account: *ctx.accounts.token_account.address(),
        token_program: *ctx.accounts.token_program.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn update_primary_sale_happened_via_token<'info>(
    ctx: CpiContext<'info, UpdatePrimarySaleHappenedViaToken<'info>>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::UpdatePrimarySaleHappenedViaToken {
        metadata: *ctx.accounts.metadata.address(),
        owner: *ctx.accounts.owner.address(),
        token: *ctx.accounts.token.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn set_token_standard<'info>(
    ctx: CpiContext<'info, SetTokenStandard<'info>>,
    edition_account: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::SetTokenStandard {
        edition: edition_account,
        metadata: *ctx.accounts.metadata_account.address(),
        mint: *ctx.accounts.mint_account.address(),
        update_authority: *ctx.accounts.update_authority.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn sign_metadata<'info>(ctx: CpiContext<'info, SignMetadata<'info>>) -> Result<()> {
    let ix = mpl_token_metadata::instructions::SignMetadata {
        creator: *ctx.accounts.creator.address(),
        metadata: *ctx.accounts.metadata.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn remove_creator_verification<'info>(
    ctx: CpiContext<'info, RemoveCreatorVerification<'info>>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::RemoveCreatorVerification {
        creator: *ctx.accounts.creator.address(),
        metadata: *ctx.accounts.metadata.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn utilize<'info>(
    ctx: CpiContext<'info, Utilize<'info>>,
    use_authority_record: Option<Pubkey>,
    burner: Option<Pubkey>,
    number_of_uses: u64,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::Utilize {
        ata_program: spl_associated_token_account_interface::program::ID,
        burner,
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        owner: *ctx.accounts.owner.address(),
        rent: solana_sysvar::rent::ID,
        system_program: anchor_lang_v2::programs::System::id(),
        token_account: *ctx.accounts.token_account.address(),
        token_program: spl_token_interface::ID,
        use_authority: *ctx.accounts.use_authority.address(),
        use_authority_record,
    }
    .instruction(mpl_token_metadata::instructions::UtilizeInstructionArgs { number_of_uses });
    ctx.invoke_ix(ix)
}

pub fn unverify_collection<'info>(
    ctx: CpiContext<'info, UnverifyCollection<'info>>,
    collection_authority_record: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::UnverifyCollection {
        collection: *ctx.accounts.collection.address(),
        collection_authority: *ctx.accounts.collection_authority.address(),
        collection_authority_record,
        collection_master_edition_account: *ctx
            .accounts
            .collection_master_edition_account
            .address(),
        collection_mint: *ctx.accounts.collection_mint.address(),
        metadata: *ctx.accounts.metadata.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub fn unverify_sized_collection_item<'info>(
    ctx: CpiContext<'info, UnverifySizedCollectionItem<'info>>,
    collection_authority_record: Option<Pubkey>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::UnverifySizedCollectionItem {
        collection: *ctx.accounts.collection.address(),
        collection_authority: *ctx.accounts.collection_authority.address(),
        collection_authority_record,
        collection_master_edition_account: *ctx
            .accounts
            .collection_master_edition_account
            .address(),
        collection_mint: *ctx.accounts.collection_mint.address(),
        metadata: *ctx.accounts.metadata.address(),
        payer: *ctx.accounts.payer.address(),
    }
    .instruction();
    ctx.invoke_ix(ix)
}

pub struct ApproveCollectionAuthority<'info> {
    pub collection_authority_record: CpiHandle<'info>,
    pub new_collection_authority: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
}

pub struct BubblegumSetCollectionSize<'info> {
    pub metadata_account: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub bubblegum_signer: CpiHandle<'info>,
}

pub struct BurnEditionNft<'info> {
    pub metadata: CpiHandle<'info>,
    pub owner: CpiHandle<'info>,
    pub print_edition_mint: CpiHandle<'info>,
    pub master_edition_mint: CpiHandle<'info>,
    pub print_edition_token: CpiHandle<'info>,
    pub master_edition_token: CpiHandle<'info>,
    pub master_edition: CpiHandle<'info>,
    pub print_edition: CpiHandle<'info>,
    pub edition_marker: CpiHandle<'info>,
    pub spl_token: CpiHandle<'info>,
}

pub struct BurnNft<'info> {
    pub metadata: CpiHandle<'info>,
    pub owner: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub token: CpiHandle<'info>,
    pub edition: CpiHandle<'info>,
    pub spl_token: CpiHandle<'info>,
}

pub struct CreateMetadataAccountsV3<'info> {
    pub metadata: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub mint_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
}

pub struct UpdateMetadataAccountsV2<'info> {
    pub metadata: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
}

pub struct CreateMasterEditionV3<'info> {
    pub edition: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub mint_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
    pub rent: CpiHandle<'info>,
}

pub struct MintNewEditionFromMasterEditionViaToken<'info> {
    pub new_metadata: CpiHandle<'info>,
    pub new_edition: CpiHandle<'info>,
    pub master_edition: CpiHandle<'info>,
    pub new_mint: CpiHandle<'info>,
    pub edition_mark_pda: CpiHandle<'info>,
    pub new_mint_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub token_account_owner: CpiHandle<'info>,
    pub token_account: CpiHandle<'info>,
    pub new_metadata_update_authority: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
    pub rent: CpiHandle<'info>,
    //
    // Not actually used by the program but still needed because it's needed
    // for the pda calculation in the helper. :/
    //
    // The better thing to do would be to remove this and have the instruction
    // helper pass in the `edition_mark_pda` directly.
    //
    pub metadata_mint: CpiHandle<'info>,
}

pub struct RevokeCollectionAuthority<'info> {
    pub collection_authority_record: CpiHandle<'info>,
    pub delegate_authority: CpiHandle<'info>,
    pub revoke_authority: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
}

pub struct SetCollectionSize<'info> {
    pub metadata: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
}

pub struct SetTokenStandard<'info> {
    pub metadata_account: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub mint_account: CpiHandle<'info>,
}

pub struct VerifyCollection<'info> {
    pub payer: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub collection_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandle<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

pub struct VerifySizedCollectionItem<'info> {
    pub payer: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub collection_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandle<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

pub struct SetAndVerifyCollection<'info> {
    pub metadata: CpiHandle<'info>,
    pub collection_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandle<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

pub struct SetAndVerifySizedCollectionItem<'info> {
    pub metadata: CpiHandle<'info>,
    pub collection_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub update_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandle<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

pub struct FreezeDelegatedAccount<'info> {
    pub metadata: CpiHandle<'info>,
    pub delegate: CpiHandle<'info>,
    pub token_account: CpiHandle<'info>,
    pub edition: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
}

pub struct ThawDelegatedAccount<'info> {
    pub metadata: CpiHandle<'info>,
    pub delegate: CpiHandle<'info>,
    pub token_account: CpiHandle<'info>,
    pub edition: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
}

pub struct UpdatePrimarySaleHappenedViaToken<'info> {
    pub metadata: CpiHandle<'info>,
    pub owner: CpiHandle<'info>,
    pub token: CpiHandle<'info>,
}

pub struct SignMetadata<'info> {
    pub creator: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
}

pub struct RemoveCreatorVerification<'info> {
    pub creator: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
}

pub struct Utilize<'info> {
    pub metadata: CpiHandle<'info>,
    pub token_account: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub use_authority: CpiHandle<'info>,
    pub owner: CpiHandle<'info>,
}

pub struct UnverifyCollection<'info> {
    pub metadata: CpiHandle<'info>,
    pub collection_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection: CpiHandle<'info>,
    pub collection_master_edition_account: CpiHandle<'info>,
}

pub struct UnverifySizedCollectionItem<'info> {
    pub metadata: CpiHandle<'info>,
    pub collection_authority: CpiHandle<'info>,
    pub payer: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection: CpiHandle<'info>,
    pub collection_master_edition_account: CpiHandle<'info>,
}

impl_cpi_accounts!(ApproveCollectionAuthority {
    collection_authority_record,
    new_collection_authority,
    update_authority,
    payer,
    metadata,
    mint
});
impl_cpi_accounts!(BubblegumSetCollectionSize {
    metadata_account,
    update_authority,
    mint,
    bubblegum_signer
});
impl_cpi_accounts!(BurnEditionNft {
    metadata,
    owner,
    print_edition_mint,
    master_edition_mint,
    print_edition_token,
    master_edition_token,
    master_edition,
    print_edition,
    edition_marker,
    spl_token
});
impl_cpi_accounts!(BurnNft {
    metadata,
    owner,
    mint,
    token,
    edition,
    spl_token
});
impl_cpi_accounts!(CreateMetadataAccountsV3 {
    metadata,
    mint,
    mint_authority,
    payer,
    update_authority,
    system_program
});
impl_cpi_accounts!(UpdateMetadataAccountsV2 {
    metadata,
    update_authority
});
impl_cpi_accounts!(CreateMasterEditionV3 {
    edition,
    mint,
    update_authority,
    mint_authority,
    payer,
    metadata,
    token_program,
    system_program,
    rent
});
impl_cpi_accounts!(MintNewEditionFromMasterEditionViaToken {
    new_metadata,
    new_edition,
    master_edition,
    new_mint,
    edition_mark_pda,
    new_mint_authority,
    payer,
    token_account_owner,
    token_account,
    new_metadata_update_authority,
    metadata,
    token_program,
    system_program,
    rent,
    metadata_mint
});
impl_cpi_accounts!(RevokeCollectionAuthority {
    collection_authority_record,
    delegate_authority,
    revoke_authority,
    metadata,
    mint
});
impl_cpi_accounts!(SetCollectionSize {
    metadata,
    mint,
    update_authority,
    system_program
});
impl_cpi_accounts!(SetTokenStandard {
    metadata_account,
    update_authority,
    mint_account
});
impl_cpi_accounts!(VerifyCollection {
    payer,
    metadata,
    collection_authority,
    collection_mint,
    collection_metadata,
    collection_master_edition
});
impl_cpi_accounts!(VerifySizedCollectionItem {
    payer,
    metadata,
    collection_authority,
    collection_mint,
    collection_metadata,
    collection_master_edition
});
impl_cpi_accounts!(SetAndVerifyCollection {
    metadata,
    collection_authority,
    payer,
    update_authority,
    collection_mint,
    collection_metadata,
    collection_master_edition
});
impl_cpi_accounts!(SetAndVerifySizedCollectionItem {
    metadata,
    collection_authority,
    payer,
    update_authority,
    collection_mint,
    collection_metadata,
    collection_master_edition
});
impl_cpi_accounts!(FreezeDelegatedAccount {
    metadata,
    delegate,
    token_account,
    edition,
    mint,
    token_program
});
impl_cpi_accounts!(ThawDelegatedAccount {
    metadata,
    delegate,
    token_account,
    edition,
    mint,
    token_program
});
impl_cpi_accounts!(UpdatePrimarySaleHappenedViaToken {
    metadata,
    owner,
    token
});
impl_cpi_accounts!(SignMetadata { metadata, creator });
impl_cpi_accounts!(RemoveCreatorVerification { metadata, creator });
impl_cpi_accounts!(Utilize {
    metadata,
    token_account,
    mint,
    use_authority,
    owner
});
impl_cpi_accounts!(UnverifyCollection {
    metadata,
    collection_authority,
    collection_mint,
    collection,
    collection_master_edition_account
});
impl_cpi_accounts!(UnverifySizedCollectionItem {
    metadata,
    collection_authority,
    payer,
    collection_mint,
    collection,
    collection_master_edition_account
});

#[derive(Clone, Debug, PartialEq)]
pub struct MetadataAccount {
    view: Option<AccountView>,
    data: mpl_token_metadata::accounts::Metadata,
}

impl MetadataAccount {
    #[inline]
    fn parse(data: &[u8]) -> Result<mpl_token_metadata::accounts::Metadata> {
        mpl_token_metadata::accounts::Metadata::safe_deserialize(data)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl AccountDeserialize for MetadataAccount {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let data = Self::parse(buf)?;
        Ok(Self { view: None, data })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl AnchorAccount for MetadataAccount {
    type Data = mpl_token_metadata::accounts::Metadata;

    fn load(view: AccountView, _program_id: &Address) -> Result<Self> {
        if !view.owned_by(&ID) {
            return Err(ProgramError::IllegalOwner);
        }
        let data_ref = view.try_borrow()?;
        let data = Self::parse(&data_ref)?;
        drop(data_ref);
        Ok(Self {
            view: Some(view),
            data,
        })
    }

    fn account(&self) -> &AccountView {
        self.view
            .as_ref()
            .expect("metadata account loaded without AccountView")
    }
}

impl Deref for MetadataAccount {
    type Target = mpl_token_metadata::accounts::Metadata;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl IdlAccountType for MetadataAccount {}

#[derive(Clone, Debug, PartialEq)]
pub struct MasterEditionAccount {
    view: Option<AccountView>,
    data: mpl_token_metadata::accounts::MasterEdition,
}

impl MasterEditionAccount {
    #[inline]
    fn parse(data: &[u8]) -> Result<mpl_token_metadata::accounts::MasterEdition> {
        mpl_token_metadata::accounts::MasterEdition::safe_deserialize(data)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl AccountDeserialize for MasterEditionAccount {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let data = Self::parse(buf)?;
        Ok(Self { view: None, data })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl AnchorAccount for MasterEditionAccount {
    type Data = mpl_token_metadata::accounts::MasterEdition;

    fn load(view: AccountView, _program_id: &Address) -> Result<Self> {
        if !view.owned_by(&ID) {
            return Err(ProgramError::IllegalOwner);
        }
        let data_ref = view.try_borrow()?;
        let data = Self::parse(&data_ref)?;
        drop(data_ref);
        Ok(Self {
            view: Some(view),
            data,
        })
    }

    fn account(&self) -> &AccountView {
        self.view
            .as_ref()
            .expect("metadata account loaded without AccountView")
    }
}

impl Deref for MasterEditionAccount {
    type Target = mpl_token_metadata::accounts::MasterEdition;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl IdlAccountType for MasterEditionAccount {}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenRecordAccount {
    view: Option<AccountView>,
    data: mpl_token_metadata::accounts::TokenRecord,
}

impl TokenRecordAccount {
    pub const LEN: usize = mpl_token_metadata::accounts::TokenRecord::LEN;

    #[inline]
    fn parse(data: &[u8]) -> Result<mpl_token_metadata::accounts::TokenRecord> {
        mpl_token_metadata::accounts::TokenRecord::safe_deserialize(data)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl AccountDeserialize for TokenRecordAccount {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let data = Self::parse(buf)?;
        Ok(Self { view: None, data })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl AnchorAccount for TokenRecordAccount {
    type Data = mpl_token_metadata::accounts::TokenRecord;

    fn load(view: AccountView, _program_id: &Address) -> Result<Self> {
        if !view.owned_by(&ID) {
            return Err(ProgramError::IllegalOwner);
        }
        let data_ref = view.try_borrow()?;
        let data = Self::parse(&data_ref)?;
        drop(data_ref);
        Ok(Self {
            view: Some(view),
            data,
        })
    }

    fn account(&self) -> &AccountView {
        self.view
            .as_ref()
            .expect("metadata account loaded without AccountView")
    }
}

impl Deref for TokenRecordAccount {
    type Target = mpl_token_metadata::accounts::TokenRecord;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl IdlAccountType for TokenRecordAccount {}

#[derive(Clone)]
pub struct Metadata;

impl Id for Metadata {
    fn id() -> Address {
        ID
    }

    const IDL_ADDRESS: &'static str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
}
