//! Allocator-neutral instruction views used by the public CPI API.

use {
    crate::{Address, Discriminator, FixedVec},
    alloc::vec::Vec,
    pinocchio::instruction::InstructionAccount,
};

/// Serializes instruction data: discriminator prefix plus the encoded
/// arguments. The caller owns the output storage so dynamic payloads do not
/// force an allocation in the on-chain API.
pub trait InstructionData: Discriminator {
    /// Append serialized instruction data to caller-selected storage.
    /// Implementations leave the original contents unchanged on failure.
    fn try_write_data<const N: usize>(&self, output: &mut FixedVec<u8, N>) -> crate::Result<()>;

    /// Owned host representation used by clients and tests.
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    fn data(&self) -> alloc::vec::Vec<u8>;
}

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

    /// Borrow instruction account metadata for the duration of a callback.
    /// On-chain instruction views pass their existing slice directly; host
    /// SDK instructions may build a compatibility view inside the callback.
    fn with_instruction_accounts<R>(&self, visit: impl FnOnce(&[InstructionAccount<'_>]) -> R)
        -> R;
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
    fn with_instruction_accounts<R>(
        &self,
        visit: impl FnOnce(&[InstructionAccount<'_>]) -> R,
    ) -> R {
        let accounts: Vec<_> = self
            .accounts
            .iter()
            .map(|account| {
                InstructionAccount::new(&account.pubkey, account.is_writable, account.is_signer)
            })
            .collect();
        visit(&accounts)
    }
}
