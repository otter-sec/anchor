/// Re-export pinocchio CPI building blocks for instruction construction.
pub use pinocchio::cpi::{Seed as CpiSeed, Signer as CpiSigner};
pub use pinocchio::instruction::{InstructionAccount, InstructionView};
#[cfg(feature = "const-rent")]
use pinocchio::sysvars::rent::{ACCOUNT_STORAGE_OVERHEAD, DEFAULT_LAMPORTS_PER_BYTE};
use {
    crate::require,
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

/// Largest `space` that won't overflow `u64` in the const rent formula.
/// In practice unreachable (Solana caps accounts at 10 MiB).
#[cfg(feature = "const-rent")]
const MAX_SAFE_SPACE: u64 = (u64::MAX / DEFAULT_LAMPORTS_PER_BYTE) - ACCOUNT_STORAGE_OVERHEAD;

/// System program `Transfer` instruction discriminant (variant index).
/// Encoded as `u32 LE` in the first 4 bytes of the instruction data.
const SYSTEM_TRANSFER_VARIANT: u8 = 2;

// Pin the protocol-mandated discriminant at compile time. Without this,
// the wire-format Kani harness below would still pass if someone edited
// `SYSTEM_TRANSFER_VARIANT` (both sides of its byte-equality reference
// the same constant).
const _: () = assert!(SYSTEM_TRANSFER_VARIANT == 2);

/// Encode a System program `Transfer` instruction body.
///
/// Extracted as a pure helper so the Kani harness in this module can verify
/// the real encoder byte-for-byte against the documented wire format. The
/// encoding is otherwise only reachable through `realloc_account`'s unsafe
/// CPI path, which Kani cannot model (the CPI syscall is opaque to CBMC).
#[inline]
fn encode_system_transfer(lamports: u64) -> [u8; 12] {
    let mut data = [0u8; 12];
    data[0] = SYSTEM_TRANSFER_VARIANT;
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data
}

/// Transfer lamports via the System program without consulting Pinocchio's
/// account borrow flags.
///
/// Safe for rent top-ups where only lamports change: the CPI writes fixed
/// account header fields, not the borrowed account data region.
#[inline]
pub(crate) fn transfer_lamports_unchecked(
    from: &AccountView,
    to: &AccountView,
    lamports: u64,
) -> Result<(), ProgramError> {
    let ix_data = encode_system_transfer(lamports);
    unsafe {
        let cpi_accounts: [pinocchio::cpi::CpiAccount; 2] = [
            pinocchio::cpi::CpiAccount::from(from),
            pinocchio::cpi::CpiAccount::from(to),
        ];
        let instruction = pinocchio::instruction::InstructionView {
            program_id: &pinocchio_system::ID,
            accounts: &[
                pinocchio::instruction::InstructionAccount::writable_signer(from.address()),
                pinocchio::instruction::InstructionAccount::writable(to.address()),
            ],
            data: &ix_data,
        };
        pinocchio::cpi::invoke_unchecked(&instruction, &cpi_accounts);
    }
    Ok(())
}

/// Compute the rent-exempt minimum balance for an account of `space` bytes.
///
/// Default path calls `Rent::get()` (picks up runtime formula changes).
/// With `const-rent` feature, uses baked-in constants (zero syscall cost
/// but locks the formula into the binary).
#[cfg(not(feature = "const-rent"))]
#[inline]
pub fn rent_exempt_lamports(space: usize) -> Result<u64, ProgramError> {
    use pinocchio::sysvars::{rent::Rent, Sysvar};
    Rent::get()?.try_minimum_balance(space)
}

#[cfg(feature = "const-rent")]
#[inline(always)]
pub fn rent_exempt_lamports(space: usize) -> Result<u64, ProgramError> {
    require!(
        space as u64 <= MAX_SAFE_SPACE,
        ProgramError::InvalidArgument
    );
    // Bounded by MAX_SAFE_SPACE → no overflow.
    Ok((ACCOUNT_STORAGE_OVERHEAD + space as u64).wrapping_mul(DEFAULT_LAMPORTS_PER_BYTE))
}

/// PDA bump-search loop. `$on_found` receives the hash bytes and bump
/// when a valid off-curve PDA is found.
#[cfg(target_os = "solana")]
macro_rules! pda_find_loop {
    ($seeds:expr, $program_id:expr, |$h:ident, $b:ident| $on_found:expr) => {{
        use solana_define_syscall::definitions::{sol_curve_validate_point, sol_sha256};
        const CURVE25519_EDWARDS: u64 = 0;
        const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

        let n = $seeds.len();
        let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];
        let mut i = 0;
        while i < n {
            unsafe { sptr.add(i).write($seeds[i]) };
            i += 1;
        }
        unsafe {
            sptr.add(n + 1).write($program_id.as_ref());
            sptr.add(n + 2).write(PDA_MARKER.as_slice());
        }
        let mut bump_arr = [u8::MAX];
        let bump_ptr = bump_arr.as_mut_ptr();
        unsafe { sptr.add(n).write(core::slice::from_raw_parts(bump_ptr, 1)) };
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 3) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();
        let mut bump: u64 = u8::MAX as u64;

        loop {
            unsafe { bump_ptr.write(bump as u8) };
            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                )
            };
            let on_curve = unsafe {
                sol_curve_validate_point(
                    CURVE25519_EDWARDS,
                    hash.as_ptr() as *const u8,
                    core::ptr::null_mut(),
                )
            };
            if on_curve != 0 {
                let $h = unsafe { hash.assume_init() };
                let $b = bump as u8;
                return $on_found;
            }
            if bump == 0 {
                break;
            }
            bump -= 1;
        }
        Err(ProgramError::InvalidSeeds)
    }};
}

/// Find a PDA and its bump seed.
#[inline(always)]
pub fn find_program_address(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
    match try_find_program_address(seeds, program_id) {
        Ok(result) => result,
        Err(_) => panic!("could not find PDA"),
    }
}

/// Find a PDA, returning an error if none exists.
#[inline(always)]
pub fn try_find_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<(Address, u8), ProgramError> {
    if seeds.len() > 16 {
        return Err(ProgramError::InvalidSeeds);
    }

    #[cfg(target_os = "solana")]
    {
        pda_find_loop!(seeds, program_id, |hash_bytes, bump| {
            Ok((Address::new_from_array(hash_bytes), bump))
        })
    }

    #[cfg(not(target_os = "solana"))]
    {
        Ok(Address::find_program_address(seeds, program_id))
    }
}

/// Find the canonical bump for `seeds` + `program_id` and verify that the
/// derived PDA equals `expected`. Returns just the bump on success.
#[inline(always)]
pub fn find_and_verify_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<u8, ProgramError> {
    if seeds.len() > 16 {
        return Err(ProgramError::InvalidSeeds);
    }

    #[cfg(target_os = "solana")]
    {
        pda_find_loop!(seeds, program_id, |hash_bytes, bump| {
            let derived = Address::new_from_array(hash_bytes);
            if pinocchio::address::address_eq(&derived, expected) {
                Ok(bump)
            } else {
                Err(ProgramError::InvalidSeeds)
            }
        })
    }

    #[cfg(not(target_os = "solana"))]
    {
        let (pda, bump) = Address::find_program_address(seeds, program_id);
        if pinocchio::address::address_eq(&pda, expected) {
            Ok(bump)
        } else {
            Err(ProgramError::InvalidSeeds)
        }
    }
}

/// Create a program-derived address (PDA) from `seeds` and `program_id`.
///
/// Uses `sol_sha256` + `sol_curve_validate_point` directly instead of
/// `sol_create_program_address`. The seeds slice should already be
/// runtime-valid and include the bump byte.
#[inline(always)]
pub fn create_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<Address, ProgramError> {
    #[cfg(target_os = "solana")]
    {
        let computed = hash_pda_seeds(seeds, program_id)?;
        check_off_curve(&computed)?;
        Ok(computed)
    }

    #[cfg(not(target_os = "solana"))]
    {
        Address::create_program_address(seeds, program_id).map_err(Into::into)
    }
}

/// Verify that `expected` matches the PDA derived from `seeds` and `program_id`.
///
/// On-chain this is hash-only and assumes the seeds are runtime-valid and the
/// bump is canonical. Off-chain it uses `Address::create_program_address`,
/// which also performs host-side seed and curve validation. For untrusted bumps
/// use `find_and_verify_program_address`. Seeds should already include the bump
/// byte.
#[inline(always)]
pub fn verify_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<(), ProgramError> {
    #[cfg(target_os = "solana")]
    {
        let computed = hash_pda_seeds(seeds, program_id)?;
        if pinocchio::address::address_eq(&computed, expected) {
            Ok(())
        } else {
            Err(ProgramError::InvalidSeeds)
        }
    }

    #[cfg(not(target_os = "solana"))]
    {
        let computed = Address::create_program_address(seeds, program_id)
            .map_err(|_| ProgramError::InvalidSeeds)?;
        if pinocchio::address::address_eq(&computed, expected) {
            Ok(())
        } else {
            Err(ProgramError::InvalidSeeds)
        }
    }
}

/// Like [`find_and_verify_program_address`] but skips `sol_curve_validate_point`.
///
/// Safe when the account was signed for (`MIN_DATA_LEN > 0` or `init`):
/// signing goes through `invoke_signed` → `create_program_address` which
/// includes the runtime's own curve check. The loop tries all 256 bumps
/// via hash-and-compare; SHA-256 collision resistance ensures only the
/// canonical bump matches.
#[inline(always)]
pub fn find_and_verify_program_address_skip_curve(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<u8, ProgramError> {
    if seeds.len() > 16 {
        return Err(ProgramError::InvalidSeeds);
    }

    #[cfg(target_os = "solana")]
    {
        use solana_define_syscall::definitions::sol_sha256;
        const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

        let n = seeds.len();
        let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];
        let mut i = 0;
        while i < n {
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        unsafe {
            sptr.add(n + 1).write(program_id.as_ref());
            sptr.add(n + 2).write(PDA_MARKER.as_slice());
        }
        let mut bump_arr = [u8::MAX];
        let bump_ptr = bump_arr.as_mut_ptr();
        unsafe { sptr.add(n).write(core::slice::from_raw_parts(bump_ptr, 1)) };
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 3) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();
        let mut bump: u64 = u8::MAX as u64;

        loop {
            unsafe { bump_ptr.write(bump as u8) };
            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                )
            };
            let h = unsafe { hash.assume_init() };
            let derived = Address::new_from_array(h);
            if pinocchio::address::address_eq(&derived, expected) {
                return Ok(bump as u8);
            }
            if bump == 0 {
                break;
            }
            bump -= 1;
        }
        Err(ProgramError::InvalidSeeds)
    }

    #[cfg(not(target_os = "solana"))]
    {
        let (pda, bump) = Address::find_program_address(seeds, program_id);
        if pinocchio::address::address_eq(&pda, expected) {
            Ok(bump)
        } else {
            Err(ProgramError::InvalidSeeds)
        }
    }
}

/// Verify that `addr` is off the Ed25519 curve (i.e. a valid PDA).
/// Returns `InvalidSeeds` if the point is on-curve.
#[cfg(target_os = "solana")]
#[inline(always)]
fn check_off_curve(addr: &Address) -> Result<(), ProgramError> {
    use solana_define_syscall::definitions::sol_curve_validate_point;
    const CURVE25519_EDWARDS: u64 = 0;
    let on_curve = unsafe {
        sol_curve_validate_point(
            CURVE25519_EDWARDS,
            addr as *const _ as *const u8,
            core::ptr::null_mut(),
        )
    };
    if on_curve == 0 {
        Err(ProgramError::InvalidSeeds)
    } else {
        Ok(())
    }
}

/// Hash seeds into a PDA address (sha256 only, no curve check).
#[cfg(target_os = "solana")]
#[inline(always)]
fn hash_pda_seeds(seeds: &[&[u8]], program_id: &Address) -> Result<Address, ProgramError> {
    use solana_define_syscall::definitions::sol_sha256;
    const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

    if seeds.len() > 17 {
        return Err(ProgramError::InvalidSeeds);
    }

    let n = seeds.len();
    let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
    let sptr = slices.as_mut_ptr() as *mut &[u8];

    let mut i = 0;
    while i < n {
        unsafe { sptr.add(i).write(seeds[i]) };
        i += 1;
    }
    unsafe {
        sptr.add(n).write(program_id.as_ref());
        sptr.add(n + 1).write(PDA_MARKER.as_slice());
    }

    let input = unsafe { core::slice::from_raw_parts(sptr, n + 2) };
    let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();

    unsafe {
        sol_sha256(
            input as *const _ as *const u8,
            input.len() as u64,
            hash.as_mut_ptr() as *mut u8,
        );
    }

    Ok(Address::new_from_array(unsafe { hash.assume_init() }))
}

/// Create a new account via system program CPI (no PDA signing).
#[inline(always)]
fn signer_from_seeds<'a>(seeds: &'a [&'a [u8]]) -> pinocchio::cpi::Signer<'a, 'a> {
    // SAFETY: Seed is repr(C) { *const u8, u64, PhantomData } = 16 bytes,
    // identical to &[u8] on SBF (*const u8, u64) = 16 bytes.
    // PhantomData is zero-sized. Static assertions verify at compile time.
    const _: () =
        assert!(core::mem::size_of::<&[u8]>() == core::mem::size_of::<pinocchio::cpi::Seed>());
    const _: () =
        assert!(core::mem::align_of::<&[u8]>() == core::mem::align_of::<pinocchio::cpi::Seed>());
    let signer_seeds: &[pinocchio::cpi::Seed] = unsafe {
        core::slice::from_raw_parts(seeds.as_ptr() as *const pinocchio::cpi::Seed, seeds.len())
    };
    pinocchio::cpi::Signer::from(signer_seeds)
}

#[doc(hidden)]
pub fn create_account_with_signers(
    payer: &AccountView,
    target: &AccountView,
    space: usize,
    owner: &Address,
    target_signer_seeds: Option<&[&[u8]]>,
    payer_signer_seeds: Option<&[&[u8]]>,
) -> Result<(), ProgramError> {
    require!(
        !pinocchio::address::address_eq(payer.address(), target.address()),
        ProgramError::InvalidArgument
    );

    let required = rent_exempt_lamports(space)?;
    let current = target.lamports();

    if current == 0 {
        let create = pinocchio_system::instructions::CreateAccount {
            from: payer,
            to: target,
            lamports: required,
            space: space as u64,
            owner,
        };
        match (payer_signer_seeds, target_signer_seeds) {
            (None, None) => create.invoke()?,
            (Some(payer_seeds), None) => {
                let payer_signer = signer_from_seeds(payer_seeds);
                create.invoke_signed(&[payer_signer])?;
            }
            (None, Some(target_seeds)) => {
                let target_signer = signer_from_seeds(target_seeds);
                create.invoke_signed(&[target_signer])?;
            }
            (Some(payer_seeds), Some(target_seeds)) => {
                let payer_signer = signer_from_seeds(payer_seeds);
                let target_signer = signer_from_seeds(target_seeds);
                create.invoke_signed(&[payer_signer, target_signer])?;
            }
        }
    } else {
        create_prefunded(
            payer,
            target,
            space,
            owner,
            required,
            current,
            target_signer_seeds,
            payer_signer_seeds,
        )?;
    }
    Ok(())
}

/// Create a new account via system program CPI (no PDA signing).
#[inline(always)]
pub fn create_account(
    payer: &AccountView,
    target: &AccountView,
    space: usize,
    owner: &Address,
) -> Result<(), ProgramError> {
    create_account_with_signers(payer, target, space, owner, None, None)
}

/// Create a new PDA account via system program CPI with signer seeds.
///
/// `seeds` should include the bump byte, e.g. `&[b"market", id.as_ref(), &[bump]]`.
#[inline(always)]
pub fn create_account_signed(
    payer: &AccountView,
    target: &AccountView,
    space: usize,
    owner: &Address,
    seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    create_account_with_signers(payer, target, space, owner, Some(seeds), None)
}

/// Rare-path fallback for when the target account already holds lamports
/// at creation time (e.g. airdropped PDAs or `init_if_needed`).
#[cold]
fn create_prefunded(
    payer: &AccountView,
    target: &AccountView,
    space: usize,
    owner: &Address,
    required: u64,
    current: u64,
    target_signer_seeds: Option<&[&[u8]]>,
    payer_signer_seeds: Option<&[&[u8]]>,
) -> Result<(), ProgramError> {
    let top_up = required.saturating_sub(current);
    if top_up > 0 {
        let transfer = pinocchio_system::instructions::Transfer {
            from: payer,
            to: target,
            lamports: top_up,
        };
        match payer_signer_seeds {
            Some(seeds) => {
                let payer_signer = signer_from_seeds(seeds);
                transfer.invoke_signed(&[payer_signer])?;
            }
            None => transfer.invoke()?,
        }
    }

    let allocate = pinocchio_system::instructions::Allocate {
        account: target,
        space: space as u64,
    };
    let assign = pinocchio_system::instructions::Assign {
        account: target,
        owner,
    };
    match target_signer_seeds {
        Some(seeds) => {
            let target_signer = signer_from_seeds(seeds);
            allocate.invoke_signed(&[target_signer])?;
            let target_signer = signer_from_seeds(seeds);
            assign.invoke_signed(&[target_signer])?;
        }
        None => {
            allocate.invoke()?;
            assign.invoke()?;
        }
    }
    Ok(())
}

/// Realloc an account to a new size, adjusting rent as needed.
///
/// Requires Pinocchio's account-resize entrypoint hook so
/// `original_data_len` tracking is available in `RuntimeAccount.padding`.
pub fn realloc_account(
    account: &mut AccountView,
    new_space: usize,
    payer: &AccountView,
    zero: bool,
) -> Result<(), ProgramError> {
    use pinocchio::Resize;

    let old_space = account.data_len();
    let required = rent_exempt_lamports(new_space)?;
    let current_lamports = account.lamports();

    if new_space > old_space {
        let deficit = required.saturating_sub(current_lamports);
        if deficit > 0 {
            transfer_lamports_unchecked(payer, &*account as &AccountView, deficit)?;
        }
    } else if new_space < old_space {
        let excess = current_lamports.saturating_sub(required);
        if excess > 0 {
            let mut payer_mut = *payer;
            // `checked_add` rather than `+`: overflow-checks is disabled in
            // release builds, and this arithmetic is on user-supplied account
            // lamports. The total SOL supply is bounded so overflow is
            // unreachable in practice, but silent wrap would be a downgrade.
            let new_payer_lamports = payer_mut
                .lamports()
                .checked_add(excess)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            payer_mut.set_lamports(new_payer_lamports);
            account.set_lamports(required);
        }
    }

    // SAFETY: resize_unchecked writes data_len (a fixed-offset field
    // before the data region) — disjoint from BorshAccount's RefMut
    // which guards data[..]. Slab's borrow_state == 0 also triggers
    // the checked path's rejection. The derive's realloc constraint
    // does release/reacquire around this call; exit() has a fallback
    // stale-length detector for non-derive callers.
    unsafe { account.resize_unchecked(new_space)? };

    if zero && new_space > old_space {
        unsafe {
            let data = account.borrow_unchecked_mut();
            for byte in &mut data[old_space..new_space] {
                *byte = 0;
            }
        }
    }

    Ok(())
}

#[cfg(all(test, feature = "const-rent"))]
mod const_rent_tests {
    use super::*;

    // `space > MAX_SAFE_SPACE` must short-circuit to an error before the
    // formula runs. Covered by unit test because the branch is boolean —
    // a couple of boundary values suffice and Kani adds no value.
    #[test]
    fn rejects_oversized_space() {
        assert!(rent_exempt_lamports(MAX_SAFE_SPACE as usize + 1).is_err());
        assert!(rent_exempt_lamports(usize::MAX).is_err());
    }

    // Base case: space == 0 charges only the fixed storage overhead.
    #[test]
    fn zero_space_returns_overhead_only() {
        assert_eq!(
            rent_exempt_lamports(0).unwrap(),
            ACCOUNT_STORAGE_OVERHEAD * DEFAULT_LAMPORTS_PER_BYTE,
        );
    }
}

// ---------------------------------------------------------------------------
// Kani proofs — CPI helper invariants
// ---------------------------------------------------------------------------

#[cfg(all(kani, feature = "const-rent"))]
mod kani_proofs_const_rent {
    use super::*;

    // For accepted inputs, the wrapping_mul does not actually wrap.
    // The comment "Bounded by MAX_SAFE_SPACE → no overflow" is load-bearing.
    #[kani::proof]
    #[kani::solver(z3)]
    fn accepted_inputs_do_not_overflow() {
        let space: usize = kani::any();
        kani::assume(space as u64 <= MAX_SAFE_SPACE);
        // The wrapping_mul in the implementation must equal the checked_mul
        // result (no overflow actually happened).
        let sum = (ACCOUNT_STORAGE_OVERHEAD + space as u64);
        let wrapping = sum.wrapping_mul(DEFAULT_LAMPORTS_PER_BYTE);
        let checked = sum.checked_mul(DEFAULT_LAMPORTS_PER_BYTE);
        assert!(checked == Some(wrapping));
    }

    // At the boundary space == MAX_SAFE_SPACE, the computed result
    // fits in u64.
    #[kani::proof]
    #[kani::solver(z3)]
    fn boundary_space_value_fits_u64() {
        let result = rent_exempt_lamports(MAX_SAFE_SPACE as usize);
        assert!(result.is_ok());
    }

    // Monotonicity: larger space ⇒ ≥ rent. (Logic invariant — required
    // by any sensible fee schedule.)
    #[kani::proof]
    #[kani::solver(z3)]
    fn rent_is_monotonic_in_space() {
        let a: usize = kani::any();
        let b: usize = kani::any();
        kani::assume(a <= b);
        kani::assume(b as u64 <= MAX_SAFE_SPACE);
        let ra = rent_exempt_lamports(a).unwrap();
        let rb = rent_exempt_lamports(b).unwrap();
        assert!(ra <= rb);
    }
}

#[cfg(kani)]
mod kani_proofs_pda {
    use super::*;

    // Length-guard only. Proves that `try_find_program_address` rejects
    // inputs with more than 16 seeds (Solana runtime cap). Does NOT
    // verify PDA derivation correctness, bump-search behavior, or seed
    // content handling — those live in the Solana runtime's own test
    // suite and are exercised end-to-end by the litesvm tests.
    #[kani::proof]
    fn seventeen_seeds_rejected() {
        let empty: &[u8] = &[];
        let seeds: [&[u8]; 17] = [empty; 17];
        let pid = Address::new_from_array([0u8; 32]);
        assert!(try_find_program_address(&seeds, &pid).is_err());
    }
}

// ---------------------------------------------------------------------------
// Kani proofs — System program Transfer wire format byte-identity
//
// `realloc_account` hand-encodes a System Transfer instruction instead of
// going through `pinocchio_system::instructions::Transfer` (to bypass the
// borrow-flag check while a BorshAccount RefMut is live). This harness
// verifies that the hand encoding matches the documented wire format:
//   [variant u32 LE] [lamports u64 LE]
// The `SYSTEM_TRANSFER_VARIANT` constant is shared with the production
// encoder — if someone edits it, both sides move together.
//
// Allocate / Assign / CreateAccount go through pinocchio-system directly,
// so their encoding lives outside anchor-v2 and isn't verified here.
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs_cpi_wire {
    use super::{encode_system_transfer, SYSTEM_TRANSFER_VARIANT};

    // Calls the real `encode_system_transfer` helper (the same one
    // `realloc_account` uses) so a layout change in the encoder — moving a
    // field, swapping endianness, truncating bytes — fails this harness.
    // Paired with the `const _: () = assert!(SYSTEM_TRANSFER_VARIANT == 2)`
    // guard near the constant definition, this proves both "the encoder
    // produces the protocol format" and "the protocol constant is correct."
    #[kani::proof]
    fn anchor_transfer_encoding_matches_wire_format() {
        let lamports: u64 = kani::any();

        let anchor_encoded = encode_system_transfer(lamports);

        // Documented wire format: [variant u32 LE] [lamports u64 LE].
        let mut wire_format = [0u8; 12];
        wire_format[0..4].copy_from_slice(&(SYSTEM_TRANSFER_VARIANT as u32).to_le_bytes());
        wire_format[4..12].copy_from_slice(&lamports.to_le_bytes());

        assert!(anchor_encoded == wire_format);
    }
}
