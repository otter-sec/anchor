//! Allocator-neutral instruction views used by the public CPI API.

use crate::Address;
use pinocchio::instruction::InstructionAccount;

/// A CPI instruction that can be inspected without taking ownership of a
/// heap-backed instruction representation.
///
/// Host-side SDK instructions implement this trait for compatibility, while
/// Pinocchio's borrowed [`InstructionView`] is the preferred on-chain input.
/// The public CPI helpers only depend on this borrowed interface, so their API
/// does not expose `solana-instruction::Instruction` or its `Vec` fields.
pub trait CpiInstruction {
    /// Program that receives the CPI.
    fn program_id(&self) -> &Address;

    /// Serialized instruction data.
    fn data(&self) -> &[u8];

    /// Visit instruction account metadata in wire order.
    fn for_each_account<'a>(&'a self, visit: &mut dyn FnMut(&'a Address, bool, bool));

    /// Borrow instruction account metadata for the duration of a callback.
    /// On-chain instruction views pass their existing slice directly; host
    /// SDK instructions may build a compatibility view inside the callback.
    fn with_instruction_accounts<R>(
        &self,
        visit: impl FnOnce(&[InstructionAccount<'_>]) -> R,
    ) -> R;
}

impl<'a, 'b, 'c, 'd> CpiInstruction for pinocchio::instruction::InstructionView<'a, 'b, 'c, 'd>
where
    'a: 'b,
{
    #[inline]
    fn program_id(&self) -> &Address {
        self.program_id
    }

    #[inline]
    fn data(&self) -> &[u8] {
        self.data
    }

    #[inline]
    fn for_each_account<'x>(&'x self, visit: &mut dyn FnMut(&'x Address, bool, bool)) {
        for account in self.accounts {
            visit(account.address, account.is_writable, account.is_signer);
        }
    }

    #[inline]
    fn with_instruction_accounts<R>(
        &self,
        visit: impl FnOnce(&[InstructionAccount<'_>]) -> R,
    ) -> R {
        visit(self.accounts)
    }
}

impl CpiInstruction for solana_instruction::Instruction {
    #[inline]
    fn program_id(&self) -> &Address {
        &self.program_id
    }

    #[inline]
    fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    fn for_each_account<'x>(&'x self, visit: &mut dyn FnMut(&'x Address, bool, bool)) {
        for account in &self.accounts {
            visit(&account.pubkey, account.is_writable, account.is_signer);
        }
    }

    #[inline]
    fn with_instruction_accounts<R>(
        &self,
        visit: impl FnOnce(&[InstructionAccount<'_>]) -> R,
    ) -> R {
        let accounts: alloc::vec::Vec<_> = self
            .accounts
            .iter()
            .map(|account| {
                InstructionAccount::new(&account.pubkey, account.is_writable, account.is_signer)
            })
            .collect();
        visit(&accounts)
    }
}
