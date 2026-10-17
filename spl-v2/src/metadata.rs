//! Metaplex Token Metadata CPI helpers and account wrappers.
//!
//! This mirrors Anchor v1's `anchor_spl::metadata` surface while using the v2
//! CPI/account primitives. Metaplex accounts are external Borsh accounts and do
//! not carry Anchor discriminators, so the account wrappers below deserialize
//! the raw Metaplex bytes directly.

extern crate alloc;

pub(crate) use mpl_token_metadata;

use {
    anchor_lang::{
        require, AccountDeserialize, AnchorAccount, CpiContext, CpiHandle, CpiHandleMut, Id,
        IdlAccountType, Result, ToCpiAccounts,
    },
    borsh::BorshDeserialize,
    core::ops::Deref,
    pinocchio::account::AccountView,
    solana_address::Address,
    solana_instruction::AccountMeta,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

pub const ID: Address = anchor_lang::address!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// Maximum lengths enforced by the Metaplex Token Metadata program.
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_SYMBOL_LENGTH: usize = 10;
pub const MAX_URI_LENGTH: usize = 200;
pub const MAX_CREATORS: usize = 5;

/// A UTF-8 string with storage determined at compile time.
///
/// This is used by the public metadata account views instead of exposing the
/// canonical Metaplex `String` fields. Values are validated when they cross
/// the account boundary, so `as_str` is infallible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedString<const N: usize> {
    bytes: [u8; N],
    len: u16,
}

impl<const N: usize> BoundedString<N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    #[inline]
    pub fn try_from_str(value: &str) -> Result<Self> {
        if value.len() > N || value.len() > u16::MAX as usize {
            return Err(ProgramError::InvalidInstructionData);
        }

        let mut result = Self::new();
        result.bytes[..value.len()].copy_from_slice(value.as_bytes());
        result.len = value.len() as u16;
        Ok(result)
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: the only constructor validates UTF-8 and the backing bytes
        // are private, so the invariant cannot be broken by callers.
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }
}

impl<const N: usize> Default for BoundedString<N> {
    fn default() -> Self {
        Self::new()
    }
}

pub type MetadataName = BoundedString<MAX_NAME_LENGTH>;
pub type MetadataSymbol = BoundedString<MAX_SYMBOL_LENGTH>;
pub type MetadataUri = BoundedString<MAX_URI_LENGTH>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Uninitialized,
    EditionV1,
    MasterEditionV1,
    ReservationListV1,
    MetadataV1,
    ReservationListV2,
    MasterEditionV2,
    EditionMarker,
    UseAuthorityRecord,
    CollectionAuthorityRecord,
    TokenOwnedEscrow,
    TokenRecord,
    MetadataDelegate,
    EditionMarkerV2,
    HolderDelegate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Creator {
    pub address: Pubkey,
    pub verified: bool,
    pub share: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Collection {
    pub verified: bool,
    pub key: Pubkey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseMethod {
    Burn,
    Multiple,
    Single,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Uses {
    pub use_method: UseMethod,
    pub remaining: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionDetails {
    V1 { size: u64 },
    V2 { padding: [u8; 8] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammableConfig {
    V1 { rule_set: Option<Pubkey> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenStandard {
    NonFungible,
    FungibleAsset,
    Fungible,
    NonFungibleEdition,
    ProgrammableNonFungible,
    ProgrammableNonFungibleEdition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataV2<'a> {
    pub name: &'a str,
    pub symbol: &'a str,
    pub uri: &'a str,
    pub seller_fee_basis_points: u16,
    pub creators: Option<&'a [Creator]>,
    pub collection: Option<Collection>,
    pub uses: Option<Uses>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenState {
    Unlocked,
    Locked,
    Listed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenDelegateRole {
    Sale,
    Transfer,
    Utility,
    Staking,
    Standard,
    LockedTransfer,
    Migration,
}

pub type CreatorBuffer = anchor_lang::FixedVec<Creator, MAX_CREATORS>;

/// Allocator-neutral view of a Metaplex metadata account.
#[derive(Clone, Debug)]
pub struct MetadataData {
    pub key: Key,
    pub update_authority: Pubkey,
    pub mint: Pubkey,
    pub name: MetadataName,
    pub symbol: MetadataSymbol,
    pub uri: MetadataUri,
    pub seller_fee_basis_points: u16,
    pub creators: Option<CreatorBuffer>,
    pub primary_sale_happened: bool,
    pub is_mutable: bool,
    pub edition_nonce: Option<u8>,
    pub token_standard: Option<TokenStandard>,
    pub collection: Option<Collection>,
    pub uses: Option<Uses>,
    pub collection_details: Option<CollectionDetails>,
    pub programmable_config: Option<ProgrammableConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MasterEditionData {
    pub key: Key,
    pub supply: u64,
    pub max_supply: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenRecordData {
    pub key: Key,
    pub bump: u8,
    pub state: TokenState,
    pub rule_set_revision: Option<u64>,
    pub delegate: Option<Pubkey>,
    pub delegate_role: Option<TokenDelegateRole>,
    pub locked_transfer: Option<Pubkey>,
}

impl<'a> DataV2<'a> {
    fn into_mpl(self) -> mpl_token_metadata::types::DataV2 {
        mpl_token_metadata::types::DataV2 {
            name: self.name.into(),
            symbol: self.symbol.into(),
            uri: self.uri.into(),
            seller_fee_basis_points: self.seller_fee_basis_points,
            creators: self.creators.map(|creators| {
                creators
                    .iter()
                    .map(|creator| mpl_token_metadata::types::Creator {
                        address: creator.address,
                        verified: creator.verified,
                        share: creator.share,
                    })
                    .collect()
            }),
            collection: self
                .collection
                .map(|collection| mpl_token_metadata::types::Collection {
                    verified: collection.verified,
                    key: collection.key,
                }),
            uses: self.uses.map(|uses| mpl_token_metadata::types::Uses {
                use_method: match uses.use_method {
                    UseMethod::Burn => mpl_token_metadata::types::UseMethod::Burn,
                    UseMethod::Multiple => mpl_token_metadata::types::UseMethod::Multiple,
                    UseMethod::Single => mpl_token_metadata::types::UseMethod::Single,
                },
                remaining: uses.remaining,
                total: uses.total,
            }),
        }
    }
}

fn collection_details_to_mpl(
    details: CollectionDetails,
) -> mpl_token_metadata::types::CollectionDetails {
    match details {
        CollectionDetails::V1 { size } => mpl_token_metadata::types::CollectionDetails::V1 { size },
        CollectionDetails::V2 { padding } => {
            mpl_token_metadata::types::CollectionDetails::V2 { padding }
        }
    }
}

impl MetadataData {
    fn from_mpl(data: mpl_token_metadata::accounts::Metadata) -> Result<Self> {
        let creators = data
            .creators
            .map(|values| {
                let mut creators = CreatorBuffer::new();
                for creator in values {
                    creators
                        .try_push(Creator {
                            address: creator.address,
                            verified: creator.verified,
                            share: creator.share,
                        })
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                }
                Ok::<CreatorBuffer, ProgramError>(creators)
            })
            .transpose()?;

        Ok(Self {
            key: key_from_mpl(data.key),
            update_authority: data.update_authority,
            mint: data.mint,
            name: MetadataName::try_from_str(&data.name)
                .map_err(|_| ProgramError::InvalidAccountData)?,
            symbol: MetadataSymbol::try_from_str(&data.symbol)
                .map_err(|_| ProgramError::InvalidAccountData)?,
            uri: MetadataUri::try_from_str(&data.uri)
                .map_err(|_| ProgramError::InvalidAccountData)?,
            seller_fee_basis_points: data.seller_fee_basis_points,
            creators,
            primary_sale_happened: data.primary_sale_happened,
            is_mutable: data.is_mutable,
            edition_nonce: data.edition_nonce,
            token_standard: data.token_standard.map(token_standard_from_mpl),
            collection: data.collection.map(|collection| Collection {
                verified: collection.verified,
                key: collection.key,
            }),
            uses: data.uses.map(uses_from_mpl),
            collection_details: data.collection_details.map(collection_details_from_mpl),
            programmable_config: data.programmable_config.map(programmable_config_from_mpl),
        })
    }
}

impl MasterEditionData {
    fn from_mpl(data: mpl_token_metadata::accounts::MasterEdition) -> Self {
        Self {
            key: key_from_mpl(data.key),
            supply: data.supply,
            max_supply: data.max_supply,
        }
    }
}

impl TokenRecordData {
    fn from_mpl(data: mpl_token_metadata::accounts::TokenRecord) -> Self {
        Self {
            key: key_from_mpl(data.key),
            bump: data.bump,
            state: token_state_from_mpl(data.state),
            rule_set_revision: data.rule_set_revision,
            delegate: data.delegate,
            delegate_role: data.delegate_role.map(token_delegate_role_from_mpl),
            locked_transfer: data.locked_transfer,
        }
    }
}

fn key_from_mpl(key: mpl_token_metadata::types::Key) -> Key {
    match key {
        mpl_token_metadata::types::Key::Uninitialized => Key::Uninitialized,
        mpl_token_metadata::types::Key::EditionV1 => Key::EditionV1,
        mpl_token_metadata::types::Key::MasterEditionV1 => Key::MasterEditionV1,
        mpl_token_metadata::types::Key::ReservationListV1 => Key::ReservationListV1,
        mpl_token_metadata::types::Key::MetadataV1 => Key::MetadataV1,
        mpl_token_metadata::types::Key::ReservationListV2 => Key::ReservationListV2,
        mpl_token_metadata::types::Key::MasterEditionV2 => Key::MasterEditionV2,
        mpl_token_metadata::types::Key::EditionMarker => Key::EditionMarker,
        mpl_token_metadata::types::Key::UseAuthorityRecord => Key::UseAuthorityRecord,
        mpl_token_metadata::types::Key::CollectionAuthorityRecord => Key::CollectionAuthorityRecord,
        mpl_token_metadata::types::Key::TokenOwnedEscrow => Key::TokenOwnedEscrow,
        mpl_token_metadata::types::Key::TokenRecord => Key::TokenRecord,
        mpl_token_metadata::types::Key::MetadataDelegate => Key::MetadataDelegate,
        mpl_token_metadata::types::Key::EditionMarkerV2 => Key::EditionMarkerV2,
        mpl_token_metadata::types::Key::HolderDelegate => Key::HolderDelegate,
    }
}

fn token_standard_from_mpl(value: mpl_token_metadata::types::TokenStandard) -> TokenStandard {
    match value {
        mpl_token_metadata::types::TokenStandard::NonFungible => TokenStandard::NonFungible,
        mpl_token_metadata::types::TokenStandard::FungibleAsset => TokenStandard::FungibleAsset,
        mpl_token_metadata::types::TokenStandard::Fungible => TokenStandard::Fungible,
        mpl_token_metadata::types::TokenStandard::NonFungibleEdition => {
            TokenStandard::NonFungibleEdition
        }
        mpl_token_metadata::types::TokenStandard::ProgrammableNonFungible => {
            TokenStandard::ProgrammableNonFungible
        }
        mpl_token_metadata::types::TokenStandard::ProgrammableNonFungibleEdition => {
            TokenStandard::ProgrammableNonFungibleEdition
        }
    }
}

fn uses_from_mpl(value: mpl_token_metadata::types::Uses) -> Uses {
    Uses {
        use_method: match value.use_method {
            mpl_token_metadata::types::UseMethod::Burn => UseMethod::Burn,
            mpl_token_metadata::types::UseMethod::Multiple => UseMethod::Multiple,
            mpl_token_metadata::types::UseMethod::Single => UseMethod::Single,
        },
        remaining: value.remaining,
        total: value.total,
    }
}

fn collection_details_from_mpl(
    value: mpl_token_metadata::types::CollectionDetails,
) -> CollectionDetails {
    match value {
        mpl_token_metadata::types::CollectionDetails::V1 { size } => CollectionDetails::V1 { size },
        mpl_token_metadata::types::CollectionDetails::V2 { padding } => {
            CollectionDetails::V2 { padding }
        }
    }
}

fn programmable_config_from_mpl(
    value: mpl_token_metadata::types::ProgrammableConfig,
) -> ProgrammableConfig {
    match value {
        mpl_token_metadata::types::ProgrammableConfig::V1 { rule_set } => {
            ProgrammableConfig::V1 { rule_set }
        }
    }
}

fn token_state_from_mpl(value: mpl_token_metadata::types::TokenState) -> TokenState {
    match value {
        mpl_token_metadata::types::TokenState::Unlocked => TokenState::Unlocked,
        mpl_token_metadata::types::TokenState::Locked => TokenState::Locked,
        mpl_token_metadata::types::TokenState::Listed => TokenState::Listed,
    }
}

fn token_delegate_role_from_mpl(
    value: mpl_token_metadata::types::TokenDelegateRole,
) -> TokenDelegateRole {
    match value {
        mpl_token_metadata::types::TokenDelegateRole::Sale => TokenDelegateRole::Sale,
        mpl_token_metadata::types::TokenDelegateRole::Transfer => TokenDelegateRole::Transfer,
        mpl_token_metadata::types::TokenDelegateRole::Utility => TokenDelegateRole::Utility,
        mpl_token_metadata::types::TokenDelegateRole::Staking => TokenDelegateRole::Staking,
        mpl_token_metadata::types::TokenDelegateRole::Standard => TokenDelegateRole::Standard,
        mpl_token_metadata::types::TokenDelegateRole::LockedTransfer => {
            TokenDelegateRole::LockedTransfer
        }
        mpl_token_metadata::types::TokenDelegateRole::Migration => TokenDelegateRole::Migration,
    }
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
        system_program: *ctx.accounts.system_program.address(),
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
///     .with_remaining_accounts(vec![ctx.accounts.collection_metadata.cpi_handle_mut().into()]);
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
    data: DataV2<'_>,
    is_mutable: bool,
    collection_details: Option<CollectionDetails>,
) -> Result<()> {
    let ix = mpl_token_metadata::instructions::CreateMetadataAccountV3 {
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        mint_authority: *ctx.accounts.mint_authority.address(),
        payer: *ctx.accounts.payer.address(),
        rent: None,
        system_program: *ctx.accounts.system_program.address(),
        update_authority: (
            *ctx.accounts.update_authority.address(),
            ctx.accounts.update_authority_is_signer,
        ),
    }
    .instruction(
        mpl_token_metadata::instructions::CreateMetadataAccountV3InstructionArgs {
            collection_details: collection_details.map(collection_details_to_mpl),
            data: data.into_mpl(),
            is_mutable,
        },
    );
    ctx.invoke_ix(ix)
}

pub fn update_metadata_accounts_v2<'info>(
    ctx: CpiContext<'info, UpdateMetadataAccountsV2<'info>>,
    new_update_authority: Option<Pubkey>,
    data: Option<DataV2<'_>>,
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
            data: data.map(DataV2::into_mpl),
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
        system_program: *ctx.accounts.system_program.address(),
        token_program: *ctx.accounts.token_program.address(),
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
        system_program: *ctx.accounts.system_program.address(),
        token_account: *ctx.accounts.token_account.address(),
        token_account_owner: *ctx.accounts.token_account_owner.address(),
        token_program: *ctx.accounts.token_program.address(),
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
    let ix = revoke_collection_authority_ix(&ctx.accounts);
    ctx.invoke_ix(ix)
}

pub fn set_collection_size<'info>(
    ctx: CpiContext<'info, SetCollectionSize<'info>>,
    collection_authority_record: Option<Pubkey>,
    size: u64,
) -> Result<()> {
    let ix = set_collection_size_ix(&ctx.accounts, collection_authority_record, size);
    ctx.invoke_ix(ix)
}

fn revoke_collection_authority_ix(
    accounts: &RevokeCollectionAuthority<'_>,
) -> solana_instruction::Instruction {
    let mut ix = mpl_token_metadata::instructions::RevokeCollectionAuthority {
        collection_authority_record: *accounts.collection_authority_record.address(),
        delegate_authority: *accounts.delegate_authority.address(),
        metadata: *accounts.metadata.address(),
        mint: *accounts.mint.address(),
        revoke_authority: *accounts.revoke_authority.address(),
    }
    .instruction();
    ix.accounts[1] = AccountMeta::new_readonly(*accounts.delegate_authority.address(), false);
    ix
}

fn set_collection_size_ix(
    accounts: &SetCollectionSize<'_>,
    collection_authority_record: Option<Pubkey>,
    size: u64,
) -> solana_instruction::Instruction {
    let mut ix = mpl_token_metadata::instructions::SetCollectionSize {
        collection_authority: *accounts.update_authority.address(),
        collection_authority_record,
        collection_metadata: *accounts.metadata.address(),
        collection_mint: *accounts.mint.address(),
    }
    .instruction(
        mpl_token_metadata::instructions::SetCollectionSizeInstructionArgs {
            set_collection_size_args: mpl_token_metadata::types::SetCollectionSizeArgs { size },
        },
    );
    ix.accounts[1] = AccountMeta::new_readonly(*accounts.update_authority.address(), true);
    ix
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
        ata_program: *ctx.accounts.ata_program.address(),
        burner,
        metadata: *ctx.accounts.metadata.address(),
        mint: *ctx.accounts.mint.address(),
        owner: *ctx.accounts.owner.address(),
        rent: *ctx.accounts.rent.address(),
        system_program: *ctx.accounts.system_program.address(),
        token_account: *ctx.accounts.token_account.address(),
        token_program: *ctx.accounts.token_program.address(),
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

#[derive(ToCpiAccounts)]
pub struct ApproveCollectionAuthority<'info> {
    pub collection_authority_record: CpiHandleMut<'info>,
    pub new_collection_authority: CpiHandle<'info>,
    #[signer]
    pub update_authority: CpiHandleMut<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub metadata: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct BubblegumSetCollectionSize<'info> {
    pub metadata_account: CpiHandleMut<'info>,
    #[signer]
    pub update_authority: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    #[signer]
    pub bubblegum_signer: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct BurnEditionNft<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub owner: CpiHandleMut<'info>,
    pub print_edition_mint: CpiHandleMut<'info>,
    pub master_edition_mint: CpiHandle<'info>,
    pub print_edition_token: CpiHandleMut<'info>,
    pub master_edition_token: CpiHandle<'info>,
    pub master_edition: CpiHandleMut<'info>,
    pub print_edition: CpiHandleMut<'info>,
    pub edition_marker: CpiHandleMut<'info>,
    pub spl_token: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct BurnNft<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub owner: CpiHandleMut<'info>,
    pub mint: CpiHandleMut<'info>,
    pub token: CpiHandleMut<'info>,
    pub edition: CpiHandleMut<'info>,
    pub spl_token: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct CreateMetadataAccountsV3<'info> {
    pub metadata: CpiHandleMut<'info>,
    pub mint: CpiHandle<'info>,
    #[signer]
    pub mint_authority: CpiHandle<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    #[signer(self.update_authority_is_signer)]
    pub update_authority: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
    #[account_meta(skip)]
    pub update_authority_is_signer: bool,
}

#[derive(ToCpiAccounts)]
pub struct UpdateMetadataAccountsV2<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub update_authority: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct CreateMasterEditionV3<'info> {
    pub edition: CpiHandleMut<'info>,
    pub mint: CpiHandleMut<'info>,
    #[signer]
    pub update_authority: CpiHandle<'info>,
    #[signer]
    pub mint_authority: CpiHandle<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub metadata: CpiHandleMut<'info>,
    pub token_program: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct MintNewEditionFromMasterEditionViaToken<'info> {
    pub new_metadata: CpiHandleMut<'info>,
    pub new_edition: CpiHandleMut<'info>,
    pub master_edition: CpiHandleMut<'info>,
    pub new_mint: CpiHandleMut<'info>,
    pub edition_mark_pda: CpiHandleMut<'info>,
    #[signer]
    pub new_mint_authority: CpiHandle<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    #[signer]
    pub token_account_owner: CpiHandle<'info>,
    pub token_account: CpiHandle<'info>,
    pub new_metadata_update_authority: CpiHandle<'info>,
    pub metadata: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct RevokeCollectionAuthority<'info> {
    pub collection_authority_record: CpiHandleMut<'info>,
    pub delegate_authority: CpiHandle<'info>,
    #[signer]
    pub revoke_authority: CpiHandleMut<'info>,
    pub metadata: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct SetCollectionSize<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub update_authority: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct SetTokenStandard<'info> {
    pub metadata_account: CpiHandleMut<'info>,
    #[signer]
    pub update_authority: CpiHandle<'info>,
    pub mint_account: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct VerifyCollection<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub collection_authority: CpiHandleMut<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandle<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct VerifySizedCollectionItem<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub collection_authority: CpiHandle<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandleMut<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct SetAndVerifyCollection<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub collection_authority: CpiHandleMut<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub update_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandle<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct SetAndVerifySizedCollectionItem<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub collection_authority: CpiHandle<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub update_authority: CpiHandle<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection_metadata: CpiHandleMut<'info>,
    pub collection_master_edition: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct FreezeDelegatedAccount<'info> {
    #[signer]
    pub delegate: CpiHandleMut<'info>,
    pub token_account: CpiHandleMut<'info>,
    pub edition: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct ThawDelegatedAccount<'info> {
    #[signer]
    pub delegate: CpiHandleMut<'info>,
    pub token_account: CpiHandleMut<'info>,
    pub edition: CpiHandle<'info>,
    pub mint: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct UpdatePrimarySaleHappenedViaToken<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub owner: CpiHandle<'info>,
    pub token: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct SignMetadata<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub creator: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct RemoveCreatorVerification<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub creator: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct Utilize<'info> {
    pub metadata: CpiHandleMut<'info>,
    pub token_account: CpiHandleMut<'info>,
    pub mint: CpiHandleMut<'info>,
    #[signer]
    pub use_authority: CpiHandleMut<'info>,
    pub owner: CpiHandle<'info>,
    pub token_program: CpiHandle<'info>,
    pub ata_program: CpiHandle<'info>,
    pub system_program: CpiHandle<'info>,
    pub rent: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct UnverifyCollection<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub collection_authority: CpiHandleMut<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection: CpiHandle<'info>,
    pub collection_master_edition_account: CpiHandle<'info>,
}

#[derive(ToCpiAccounts)]
pub struct UnverifySizedCollectionItem<'info> {
    pub metadata: CpiHandleMut<'info>,
    #[signer]
    pub collection_authority: CpiHandle<'info>,
    #[signer]
    pub payer: CpiHandleMut<'info>,
    pub collection_mint: CpiHandle<'info>,
    pub collection: CpiHandleMut<'info>,
    pub collection_master_edition_account: CpiHandle<'info>,
}

#[derive(Clone, Debug)]
pub struct MetadataAccount {
    view: Option<AccountView>,
    data: MetadataData,
}

impl MetadataAccount {
    #[inline]
    fn deserialize_trailing<T>(buf: &mut &[u8]) -> Result<T>
    where
        T: BorshDeserialize + Default,
    {
        let start = *buf;
        let mut cursor = start;
        match BorshDeserialize::deserialize(&mut cursor) {
            Ok(value) => {
                *buf = cursor;
                Ok(value)
            }
            Err(_) if cursor.len() == start.len() => Ok(T::default()),
            Err(_) => Err(ProgramError::InvalidAccountData),
        }
    }

    #[inline]
    fn parse(buf: &mut &[u8]) -> Result<mpl_token_metadata::accounts::Metadata> {
        use mpl_token_metadata::types::{
            Collection, CollectionDetails, Data, Key, ProgrammableConfig, TokenStandard, Uses,
        };

        if buf.is_empty() || buf[0] != Key::MetadataV1 as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        // Metaplex stores MetadataV1 accounts on chain in this exact order:
        // key, update_authority, mint, data, primary_sale_happened,
        // is_mutable, edition_nonce, token_standard, collection, uses,
        // collection_details, programmable_config.
        let key: Key =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let update_authority: Pubkey =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let mint: Pubkey =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let data: Data =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let primary_sale_happened: bool =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let is_mutable: bool =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let edition_nonce: Option<u8> =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;

        let token_standard: Option<TokenStandard> = Self::deserialize_trailing(buf)?;
        let collection: Option<Collection> = Self::deserialize_trailing(buf)?;
        let uses: Option<Uses> = Self::deserialize_trailing(buf)?;
        let collection_details: Option<CollectionDetails> = Self::deserialize_trailing(buf)?;
        let programmable_config: Option<ProgrammableConfig> = Self::deserialize_trailing(buf)?;

        Ok(mpl_token_metadata::accounts::Metadata {
            key,
            update_authority,
            mint,
            name: data.name,
            symbol: data.symbol,
            uri: data.uri,
            seller_fee_basis_points: data.seller_fee_basis_points,
            creators: data.creators,
            primary_sale_happened,
            is_mutable,
            edition_nonce,
            token_standard,
            collection,
            uses,
            collection_details,
            programmable_config,
        })
    }
}

impl AccountDeserialize for MetadataAccount {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let data = MetadataData::from_mpl(Self::parse(buf)?)?;
        Ok(Self { view: None, data })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl AnchorAccount for MetadataAccount {
    type Data = MetadataData;

    fn load(view: AccountView) -> Result<Self> {
        require!(view.owned_by(&ID), ProgramError::IllegalOwner);
        let data_ref = view.try_borrow()?;
        let data = MetadataData::from_mpl(Self::parse(&mut &data_ref[..])?)?;
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
    type Target = MetadataData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl IdlAccountType for MetadataAccount {}

#[derive(Clone, Debug, PartialEq)]
pub struct MasterEditionAccount {
    view: Option<AccountView>,
    data: MasterEditionData,
}

impl MasterEditionAccount {
    #[inline]
    fn parse(buf: &mut &[u8]) -> Result<mpl_token_metadata::accounts::MasterEdition> {
        if buf.is_empty() || buf[0] != mpl_token_metadata::types::Key::MasterEditionV2 as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        mpl_token_metadata::accounts::MasterEdition::deserialize(buf)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl AccountDeserialize for MasterEditionAccount {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let data = MasterEditionData::from_mpl(Self::parse(buf)?);
        Ok(Self { view: None, data })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl AnchorAccount for MasterEditionAccount {
    type Data = MasterEditionData;

    fn load(view: AccountView) -> Result<Self> {
        require!(view.owned_by(&ID), ProgramError::IllegalOwner);
        let data_ref = view.try_borrow()?;
        let data = MasterEditionData::from_mpl(Self::parse(&mut &data_ref[..])?);
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
    type Target = MasterEditionData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl IdlAccountType for MasterEditionAccount {}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenRecordAccount {
    view: Option<AccountView>,
    data: TokenRecordData,
}

impl TokenRecordAccount {
    pub const LEN: usize = mpl_token_metadata::accounts::TokenRecord::LEN;
    const LEGACY_LEN: usize = Self::LEN - 33;

    #[inline]
    fn parse(buf: &mut &[u8]) -> Result<mpl_token_metadata::accounts::TokenRecord> {
        const LOCKED_TRANSFER_SIZE: i64 = 33;

        let length = Self::LEN as i64 - buf.len() as i64;
        if !(length == 0 || length == LOCKED_TRANSFER_SIZE)
            || buf.first().copied() != Some(mpl_token_metadata::types::Key::TokenRecord as u8)
        {
            return Err(ProgramError::InvalidAccountData);
        }

        let key =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let bump =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let state =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let rule_set_revision =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let delegate =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let delegate_role =
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let locked_transfer = if length == 0 {
            BorshDeserialize::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData)?
        } else {
            None
        };

        Ok(mpl_token_metadata::accounts::TokenRecord {
            key,
            bump,
            state,
            rule_set_revision,
            delegate,
            delegate_role,
            locked_transfer,
        })
    }

    #[inline]
    fn parse_prefix(data: &[u8]) -> Result<(mpl_token_metadata::accounts::TokenRecord, usize)> {
        // Token records exist in both the current fixed-size layout and an
        // older shorter layout without the `locked_transfer` tail. Client-side
        // `AccountDeserialize` must consume exactly one record and leave any
        // following bytes untouched, so decode the first valid token-record
        // prefix and report its consumed length.
        if data.len() >= Self::LEN {
            let mut prefix = &data[..Self::LEN];
            if let Ok(record) = Self::parse(&mut prefix) {
                return Ok((record, Self::LEN));
            }
        }

        if data.len() < Self::LEGACY_LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let mut prefix = &data[..Self::LEGACY_LEN];
        let record = Self::parse(&mut prefix)?;
        Ok((record, Self::LEGACY_LEN))
    }
}

impl AccountDeserialize for TokenRecordAccount {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let (data, consumed) = Self::parse_prefix(buf)?;
        *buf = &buf[consumed..];
        Ok(Self {
            view: None,
            data: TokenRecordData::from_mpl(data),
        })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl AnchorAccount for TokenRecordAccount {
    type Data = TokenRecordData;

    fn load(view: AccountView) -> Result<Self> {
        require!(view.owned_by(&ID), ProgramError::IllegalOwner);
        let data_ref = view.try_borrow()?;
        let (data, _) = Self::parse_prefix(&data_ref[..])?;
        drop(data_ref);
        Ok(Self {
            view: Some(view),
            data: TokenRecordData::from_mpl(data),
        })
    }

    fn account(&self) -> &AccountView {
        self.view
            .as_ref()
            .expect("metadata account loaded without AccountView")
    }
}

impl Deref for TokenRecordAccount {
    type Target = TokenRecordData;

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

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::{
        testing::{AccountBuffer, MIN_ACCOUNT_BUF},
        ToCpiHandle, ToCpiHandleMut,
    };

    fn account(
        address: [u8; 32],
        signer: bool,
        writable: bool,
    ) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
        let buffer = AccountBuffer::new();
        buffer.init(address, [1; 32], 8, signer, writable, false);
        buffer
    }

    #[test]
    fn revoke_collection_authority_keeps_delegate_readonly() {
        let record_buffer = account([1; 32], false, true);
        let delegate_buffer = account([2; 32], false, false);
        let revoke_buffer = account([3; 32], true, true);
        let metadata_buffer = account([4; 32], false, false);
        let mint_buffer = account([5; 32], false, false);

        let mut record_view = unsafe { record_buffer.view() };
        let delegate_view = unsafe { delegate_buffer.view() };
        let mut revoke_view = unsafe { revoke_buffer.view() };
        let metadata_view = unsafe { metadata_buffer.view() };
        let mint_view = unsafe { mint_buffer.view() };

        let accounts = RevokeCollectionAuthority {
            collection_authority_record: record_view.to_cpi_handle_mut(),
            delegate_authority: delegate_view.to_cpi_handle(),
            revoke_authority: revoke_view.to_cpi_handle_mut(),
            metadata: metadata_view.to_cpi_handle(),
            mint: mint_view.to_cpi_handle(),
        };

        let metas = accounts.to_instruction_accounts();
        assert!(metas[0].is_writable);
        assert!(!metas[1].is_writable);
        assert!(metas[2].is_writable);

        let handles = accounts.to_cpi_handles();
        assert!(handles[0].is_writable());
        assert!(!handles[1].is_writable());
        assert!(handles[2].is_writable());

        let ix = revoke_collection_authority_ix(&accounts);
        assert!(ix.accounts[0].is_writable);
        assert!(!ix.accounts[1].is_writable);
        assert!(ix.accounts[2].is_writable);
    }

    #[test]
    fn set_collection_size_keeps_update_authority_readonly() {
        let metadata_buffer = account([6; 32], false, true);
        let update_authority_buffer = account([7; 32], true, false);
        let mint_buffer = account([8; 32], false, false);

        let mut metadata_view = unsafe { metadata_buffer.view() };
        let update_authority_view = unsafe { update_authority_buffer.view() };
        let mint_view = unsafe { mint_buffer.view() };

        let accounts = SetCollectionSize {
            metadata: metadata_view.to_cpi_handle_mut(),
            update_authority: update_authority_view.to_cpi_handle(),
            mint: mint_view.to_cpi_handle(),
        };

        let metas = accounts.to_instruction_accounts();
        assert!(metas[0].is_writable);
        assert!(!metas[1].is_writable);
        assert!(metas[1].is_signer);

        let handles = accounts.to_cpi_handles();
        assert!(handles[0].is_writable());
        assert!(!handles[1].is_writable());

        let ix = set_collection_size_ix(&accounts, None, 9);
        assert!(ix.accounts[0].is_writable);
        assert!(!ix.accounts[1].is_writable);
        assert!(ix.accounts[1].is_signer);
    }
}
