#![cfg_attr(target_os = "solana", feature(asm_experimental_arch))]

pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("55555555555555555555555555555555555555555555");

// Link the hand-written asm entrypoint at `src/asm/entrypoint.s`.
// `build.rs` concatenates the directory into `$OUT_DIR/combined.s` and
// `include_asm!()` emits a `global_asm!` linking it.
#[cfg(target_os = "solana")]
anchor_asm_v2_runtime::include_asm!();

/// ed25519 pubkey of `Keypair::new_from_array([7u8; 32])` — the tests
/// and bench builders reconstruct the same seed.
pub const UPDATE_AUTHORITY: Address = Address::new_from_array([
    234, 74, 108, 99, 226, 156, 82, 10, 190, 245, 80, 123, 19, 46, 197, 249, 149, 71, 118, 174,
    190, 190, 123, 146, 66, 30, 234, 105, 20, 70, 210, 44,
]);

#[program]
pub mod prop_amm_v2 {
    use super::*;

    /// Unreachable: the asm entrypoint handles `discrim=0` before this
    /// match arm runs. Declared so the macro emits `instruction::Update`
    /// + `accounts::Update` for clients and IDL.
    #[discrim = 0]
    #[allow(unused_variables)]
    pub fn update(ctx: &mut Context<Update>, new_price: u64) -> Result<()> {
        unsafe { core::hint::unreachable_unchecked() }
    }

    #[discrim = 1]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.oracle.authority = *ctx.accounts.payer.address();
        ctx.accounts.oracle.price = 0;
        Ok(())
    }

    #[discrim = 2]
    pub fn rotate_authority(
        ctx: &mut Context<RotateAuthority>,
        new_authority: [u8; 32],
    ) -> Result<()> {
        ctx.accounts.oracle.authority = Address::new_from_array(new_authority);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update {
    #[account(mut)]
    pub oracle: Account<state::Oracle>,
    pub authority: Signer,
}

// The asm entrypoint handles `discrim=0` inline and `call`s
// `__anchor_dispatch` (emitted by the `#[program]` macro under
// `no-entrypoint` with `#[no_mangle]`) for any other discriminator.
// `no-entrypoint` suppresses the macro's entrypoint + allocator + panic
// handler, so we reinstate the latter two.
//
// Offsets are hardcoded against agave's aligned serialization, which
// lays account records out as:
//   [RuntimeAccount 88B | data | MAX_PERMITTED_DATA_INCREASE 10240B
//    | padding-to-8 | rent_epoch 8B]
// For an oracle with 48B data:
//   input + 0      num_accounts: u64
//   input + 8      account 0 (oracle) header
//   input + 10     oracle.is_writable
//   input + 88     oracle.data_len
//   input + 96     oracle.data[0..48]
//   input + 136    oracle.data[40..48]  (price slot)
//   input + 10392  account 1 (authority) header
//                  = 8 + 88 + 48 + 10240 + 8
//   input + 10393  authority.is_signer
//   input + 10400  authority.address[0..32]

#[cfg(target_os = "solana")]
anchor_lang::pinocchio::default_allocator!();
#[cfg(target_os = "solana")]
anchor_lang::pinocchio::default_panic_handler!();
