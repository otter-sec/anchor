use anchor_lang::{
    prelude::Result,
    solana_program::{
        account_info::AccountInfo,
        instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT},
        program::invoke,
        pubkey::Pubkey,
        rent::Rent,
        system_instruction::transfer,
    },
    Lamports,
};
use anchor_spl::token_interface::spl_token_2022::{
    extension::{BaseStateWithExtensions, Extension, StateWithExtensions},
    state::Mint,
};
use solana_sysvar::Sysvar;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
use spl_type_length_value::variable_len_pack::VariableLenPack;

pub const APPROVE_ACCOUNT_SEED: &[u8] = b"approve-account";
pub const META_LIST_ACCOUNT_SEED: &[u8] = b"extra-account-metas";

pub fn update_account_lamports_to_minimum_balance<'info>(
    account: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
) -> Result<()> {
    let extra_lamports = Rent::get()?.minimum_balance(account.data_len()) - account.get_lamports();
    if extra_lamports > 0 {
        invoke(
            &transfer(payer.key, account.key, extra_lamports),
            &[payer, account, system_program],
        )?;
    }
    Ok(())
}

pub fn get_mint_extensible_extension_data<T: Extension + VariableLenPack>(
    account: &mut AccountInfo,
) -> Result<T> {
    let mint_data = account.data.borrow();
    let mint_with_extension = StateWithExtensions::<Mint>::unpack(&mint_data)?;
    let extension_data = mint_with_extension.get_variable_len_extension::<T>()?;
    Ok(extension_data)
}

pub fn get_extra_meta_list_account_pda(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[META_LIST_ACCOUNT_SEED, mint.as_ref()], &crate::id()).0
}

pub fn get_approve_account_pda(mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[APPROVE_ACCOUNT_SEED, mint.as_ref()], &crate::id()).0
}

/// Determine if we are in CPI
pub fn hook_in_cpi() -> bool {
    let stack_height = get_stack_height();
    let tx_height = TRANSACTION_LEVEL_STACK_HEIGHT;
    let hook_height: usize = tx_height + 1;

    stack_height > hook_height
}

pub fn get_meta_list(approve_account: Option<Pubkey>) -> Vec<ExtraAccountMeta> {
    if let Some(approve_account) = approve_account {
        return vec![ExtraAccountMeta {
            discriminator: 0,
            address_config: approve_account.to_bytes(),
            is_signer: false.into(),
            is_writable: true.into(),
        }];
    }
    vec![]
}

pub fn get_meta_list_size(approve_account: Option<Pubkey>) -> usize {
    // safe because it's either 0 or 1
    ExtraAccountMetaList::size_of(get_meta_list(approve_account).len()).unwrap()
}
