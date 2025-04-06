pub use crate::token_2022::*;
#[cfg(feature = "token_2022_extensions")]
pub use crate::token_2022_extensions::*;
use {
    anchor_lang::{
        __private::bytemuck::Pod,
        solana_program::{program_pack::Pack, pubkey::Pubkey},
    },
    spl_token_2022::extension::{
        BaseStateWithExtensions, Extension, ExtensionType, StateWithExtensions,
    },
    std::ops::Deref,
};

static IDS: [Pubkey; 2] = [spl_token_interface::ID, spl_token_2022_interface::ID];

#[derive(Clone, Debug, Default, PartialEq, Copy)]
pub struct TokenAccount(spl_token_2022::state::Account);

impl anchor_lang::AccountDeserialize for TokenAccount {
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
            buf,
        )
        .map(|t| TokenAccount(t.base))
        .map_err(Into::into)
    }
}

impl anchor_lang::AccountSerialize for TokenAccount {}

impl anchor_lang::Owners for TokenAccount {
    fn owners() -> &'static [Pubkey] {
        &IDS
    }
}

impl Deref for TokenAccount {
    type Target = spl_token_2022::state::Account;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Copy)]
pub struct Mint(spl_token_2022::state::Mint);

impl anchor_lang::AccountDeserialize for Mint {
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(buf)
            .map(|t| Mint(t.base))
            .map_err(Into::into)
    }
}

impl anchor_lang::AccountSerialize for Mint {}

impl anchor_lang::Owners for Mint {
    fn owners() -> &'static [Pubkey] {
        &IDS
    }
}

impl Deref for Mint {
    type Target = spl_token_2022::state::Mint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct TokenInterface;

impl anchor_lang::Ids for TokenInterface {
    fn ids() -> &'static [Pubkey] {
        &IDS
    }
}

pub type ExtensionsVec = Vec<ExtensionType>;

pub fn find_mint_account_size(extensions: Option<&ExtensionsVec>) -> anchor_lang::Result<usize> {
    if let Some(extensions) = extensions {
        Ok(ExtensionType::try_calculate_account_len::<
            spl_token_2022::state::Mint,
        >(extensions)?)
    } else {
        Ok(spl_token_2022::state::Mint::LEN)
    }
}

pub fn get_mint_extension_data<T: Extension + Pod>(
    account: &anchor_lang::solana_program::account_info::AccountInfo,
) -> anchor_lang::Result<T> {
    let mint_data = account.data.borrow();
    let mint_with_extension =
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
    let extension_data = *mint_with_extension.get_extension::<T>()?;
    Ok(extension_data)
}

pub fn get_token_account_extension_data<T: Extension + Pod>(
    account: &anchor_lang::solana_program::account_info::AccountInfo,
) -> anchor_lang::Result<T> {
    let token_account_data = account.data.borrow();
    let token_account_with_extension =
        StateWithExtensions::<spl_token_2022::state::Account>::unpack(&token_account_data)?;
    let extension_data = *token_account_with_extension.get_extension::<T>()?;
    Ok(extension_data)
}
