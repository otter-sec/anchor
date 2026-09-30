use anchor_lang::{prelude::*, AccountDeserialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account(zero_copy)]
pub struct UnalignedZeroCopy {
    pub value: u128,
}

// Explicit bytemuck derives must suppress the injected ones (E0119 otherwise).
#[account(zero_copy)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExplicitBytemuckDerives {
    pub value: u64,
}

// Deriving only Zeroable must still get Pod injected. Compile-only check.
#[account(zero_copy)]
#[derive(bytemuck::Zeroable)]
pub struct OnlyZeroableDerived {
    pub value: u64,
}

#[test]
fn zero_copy_accepts_explicit_bytemuck_derives() {
    let account = ExplicitBytemuckDerives { value: 7 };
    let bytes = anchor_lang::__private::bytemuck::bytes_of(&account);
    assert_eq!(bytes, 7u64.to_le_bytes());
}

#[test]
fn zero_copy_try_deserialize_handles_unaligned_bytes() {
    let account = UnalignedZeroCopy { value: 42 };
    let mut raw = Vec::with_capacity(
        1 + UnalignedZeroCopy::DISCRIMINATOR.len() + core::mem::size_of::<UnalignedZeroCopy>(),
    );
    raw.push(0);
    raw.extend_from_slice(UnalignedZeroCopy::DISCRIMINATOR);
    raw.extend_from_slice(anchor_lang::__private::bytemuck::bytes_of(&account));

    let mut data: &[u8] = &raw[1..];
    let deserialized = UnalignedZeroCopy::try_deserialize(&mut data).unwrap();

    assert_eq!(deserialized.value, account.value);
}
