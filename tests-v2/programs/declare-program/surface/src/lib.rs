use anchor_lang::prelude::*;

declare_id!("D9t6cEFPTDWmTZfcikokLbnuuyeJT6oXnpEbyXB45LU2");

declare_program!(surface);

// Compile the target-side generated forms directly. This proves that dynamic
// IDL fields borrow their input instead of requiring `String` or `Vec` in a
// program crate.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[allow(dead_code)]
fn borrowed_dynamic_surface<'ix>(
    label: Option<&'ix str>,
    keys: &'ix [Address],
) -> surface::SurfaceMode<'ix> {
    surface::SurfaceMode::Named {
        authority: Address::new_from_array([1; 32]),
        args: surface::SurfaceArgs {
            enabled: true,
            small: -1,
            count: 2,
            maybe_label: label,
            keys,
            pair: [3, 4],
        },
    }
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[allow(dead_code)]
fn write_borrowed_dynamic_instruction<'ix, const N: usize>(
    amounts: &'ix [u16],
    label: Option<&'ix str>,
    keys: &'ix [Address],
    output: &mut FixedVec<u8, N>,
) -> Result<()> {
    let args = || surface::SurfaceArgs {
        enabled: true,
        small: -1,
        count: 2,
        maybe_label: label,
        keys,
        pair: [3, 4],
    };
    surface::instruction::ComplexArgs {
        amounts,
        maybe_owner: None,
        fixed: [5, 6, 7, 8],
        by_object: args(),
        by_string: args(),
        by_enum: surface::SurfaceMode::Named {
            authority: Address::new_from_array([1; 32]),
            args: args(),
        },
        by_tuple: surface::TupleFlag(true),
    }
    .try_write_data(output)
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[allow(dead_code)]
fn invoke_borrowed_dynamic_instruction<'a, 'ix, const DATA: usize>(
    program: &'a Address,
    amounts: &'ix [u16],
    label: Option<&'ix str>,
    keys: &'ix [Address],
    output: &mut FixedVec<u8, DATA>,
) -> Result<()> {
    let args = || surface::SurfaceArgs {
        enabled: true,
        small: -1,
        count: 2,
        maybe_label: label,
        keys,
        pair: [3, 4],
    };
    surface::cpi::complex_args::<0, DATA>(
        CpiContext::<_, 0>::new(program, surface::cpi::accounts::ComplexArgs::new()),
        amounts,
        None,
        [5, 6, 7, 8],
        args(),
        args(),
        surface::SurfaceMode::Named {
            authority: Address::new_from_array([1; 32]),
            args: args(),
        },
        surface::TupleFlag(true),
        output,
    )
}
