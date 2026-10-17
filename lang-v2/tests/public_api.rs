#![no_std]

extern crate std;

use anchor_lang::{
    cpi::{InstructionAccount, InstructionView},
    AccountMeta, AccountMetaBuffer, Address, CpiInstruction, Discriminator, FixedVec,
    InstructionData, ToAccountMetas,
};

struct Accounts {
    address: Address,
}

impl ToAccountMetas for Accounts {
    fn to_account_metas(&self, is_signer: Option<bool>) -> AccountMetaBuffer {
        let mut metas = AccountMetaBuffer::new();
        metas
            .try_push(AccountMeta {
                pubkey: self.address,
                is_signer: is_signer.unwrap_or(false),
                is_writable: true,
            })
            .unwrap();
        metas
    }
}

struct Args;

impl Discriminator for Args {
    const DISCRIMINATOR: &'static [u8] = &[1, 2, 3, 4];
}

impl InstructionData for Args {
    fn data(&self) -> anchor_lang::InstructionDataBuffer {
        let mut data = anchor_lang::InstructionDataBuffer::new();
        data.extend_from_slice(Self::DISCRIMINATOR);
        data
    }
}

fn accepts_borrowed_instruction<I: CpiInstruction + ?Sized>(instruction: &I) -> usize {
    let mut account_count = 0;
    instruction.for_each_account(&mut |_, _, _| account_count += 1);
    account_count
}

#[test]
fn public_api_is_available_without_importing_alloc_types() {
    let program_id = Address::default();
    let account = Address::default();
    let accounts = Accounts { address: account };
    let metas = accounts.to_account_metas(None);
    assert_eq!(metas.len(), 1);
    assert_eq!(Args.data().len(), 4);

    let instruction_accounts = [InstructionAccount::readonly(&account)];
    let instruction = InstructionView {
        program_id: &program_id,
        accounts: &instruction_accounts,
        data: Args::DISCRIMINATOR,
    };
    assert_eq!(accepts_borrowed_instruction(&instruction), 1);

    let _ = FixedVec::<u8, 4>::new();
}
