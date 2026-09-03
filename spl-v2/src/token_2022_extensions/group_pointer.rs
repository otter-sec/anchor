use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_instruction::Instruction,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct GroupPointerInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct GroupPointerUpdate<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

pub fn group_pointer_initialize<'a>(
    ctx: CpiContext<'a, GroupPointerInitialize<'a>>,
    authority: Option<&Address>,
    group_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::group_pointer::instruction::initialize(
        ctx.program,
        ctx.accounts.mint.address(),
        authority.copied(),
        group_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn group_pointer_update<'a>(
    ctx: CpiContext<'a, GroupPointerUpdate<'a>>,
    group_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = group_pointer_update_ix(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        group_address,
    )?;
    ctx.invoke_ix(ix)
}

fn group_pointer_update_ix(
    program: &Address,
    mint: &Address,
    authority: &Address,
    signer_addresses: &[&Address],
    group_address: Option<&Address>,
) -> Result<Instruction, ProgramError> {
    spl_token_2022::extension::group_pointer::instruction::update(
        program,
        mint,
        authority,
        signer_addresses,
        group_address.copied(),
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        anchor_lang::{
            programs::Token2022,
            testing::{AccountBuffer, MIN_ACCOUNT_BUF},
            Address, Id,
        },
    };

    fn account(
        address: [u8; 32],
        signer: bool,
        writable: bool,
    ) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
        let buffer = AccountBuffer::new();
        buffer.init(address, [9; 32], 8, signer, writable, false);
        buffer
    }

    #[test]
    fn group_pointer_update_uses_single_authority_layout() {
        let program = Token2022::id();
        let mint = Address::new_from_array([1; 32]);
        let authority = Address::new_from_array([2; 32]);
        let group = Address::new_from_array([3; 32]);

        let ix = group_pointer_update_ix(&program, &mint, &authority, &[], Some(&group))
            .expect("group pointer update ix should build");
        assert_eq!(ix.accounts.len(), 2);
        assert!(ix.accounts[0].is_writable);
        assert!(!ix.accounts[0].is_signer);
        assert_eq!(ix.accounts[0].pubkey.as_ref(), mint.as_ref());
        assert!(!ix.accounts[1].is_writable);
        assert!(ix.accounts[1].is_signer);
        assert_eq!(ix.accounts[1].pubkey.as_ref(), authority.as_ref());
    }

    #[test]
    fn group_pointer_update_accounts_do_not_duplicate_authority() {
        let mint_buffer = account([1; 32], false, true);
        let authority_buffer = account([2; 32], true, false);
        let mut mint_view = unsafe { mint_buffer.view() };
        let authority_view = unsafe { authority_buffer.view() };

        let accounts = GroupPointerUpdate {
            mint: CpiHandleMut::writable(&mut mint_view),
            authority: CpiHandle::readonly(&authority_view),
        };

        assert_eq!(accounts.to_cpi_handles().len(), 2);
    }
}
