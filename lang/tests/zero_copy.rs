use anchor_lang::{prelude::*, AccountDeserialize};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account(zero_copy)]
pub struct UnalignedZeroCopy {
    pub value: u128,
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
