// Avoiding AccountInfo deprecated msg in anchor context
#![allow(dead_code, deprecated)]
// Generic accounts are not supported with `Lazy`
#![cfg(not(feature = "lazy-account"))]

use {
    anchor_lang::prelude::{borsh::io::Write, *},
    borsh::{BorshDeserialize, BorshSerialize},
    solana_pubkey::Pubkey,
};

// Needed to declare accounts.
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[derive(Accounts)]
pub struct CustomLifetime<'a> {
    pub non_generic: UncheckedAccount<'a>,
}

#[derive(Accounts)]
pub struct GenericsTest<'info, T, U, const N: usize>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
    U: BorshSerialize + BorshDeserialize + Default + Clone,
{
    pub non_generic: UncheckedAccount<'info>,
    pub generic: Account<'info, T>,

    pub const_generic: AccountLoader<'info, FooAccount<N>>,
    pub const_generic_loader: AccountLoader<'info, FooAccount<N>>,
    pub associated: Account<'info, Associated<U>>,
}

#[account(zero_copy(unsafe))]
pub struct FooAccount<const N: usize> {
    pub data: WrappedU8Array<N>,
}

#[account]
#[derive(Default)]
pub struct Associated<T>
where
    T: BorshDeserialize + BorshSerialize + Clone + Default,
{
    pub data: T,
}

#[derive(Copy, Clone)]
pub struct WrappedU8Array<const N: usize>(u8);
impl<const N: usize> BorshSerialize for WrappedU8Array<N> {
    fn serialize<W: Write>(&self, _writer: &mut W) -> borsh::io::Result<()> {
        todo!()
    }
}
impl<const N: usize> BorshDeserialize for WrappedU8Array<N> {
    fn deserialize(_buf: &mut &[u8]) -> borsh::io::Result<Self> {
        todo!()
    }

    fn deserialize_reader<R: std::io::Read>(_reader: &mut R) -> std::io::Result<Self> {
        todo!()
    }
}
impl<const N: usize> Owner for WrappedU8Array<N> {
    fn owner() -> Pubkey {
        crate::ID
    }
}
