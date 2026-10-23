#![cfg_attr(any(target_os = "solana", target_arch = "bpf"), no_std)]

use anchor_lang::prelude::*;

declare_id!("4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi");

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

/// This fixture intentionally keeps every operation in one generated
/// entrypoint. It is compiled for bpfel-unknown-none with no default features
/// and is not an end-to-end protocol test.
#[program]
pub mod no_alloc_proof {
    use super::*;

    pub fn exercise(ctx: &mut Context<Exercise>) -> Result<()> {
        let _remaining = ctx.remaining_accounts();
        msg!("no-alloc proof");

        let sequence = BoundedSequence {
            bytes: anchor_lang::__idl::vec::Vec::new(),
            text: anchor_lang::__idl::string::String::new(),
        };
        let _ = sequence;

        let event = ProofEvent { marker: 7 };
        let _event_data = event.try_data()?;
        emit_cpi!(ProofEvent { marker: 7 });

        let signer_seed_group: &[&[u8]] = &[b"proof"];
        let signer_seeds: &[&[&[u8]]] = &[signer_seed_group];

        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.payer.cpi_handle_mut(),
            to: ctx.accounts.destination.cpi_handle_mut(),
        };
        let mut remaining = anchor_lang::CpiHandleBuffer::new();
        remaining
            .try_push(ctx.accounts.spectator.cpi_handle())
            .map_err(|_| Error::InvalidArgument)?;
        let cpi_context = CpiContext::<_, 3>::new_with_signer_capacity(
            ctx.accounts.system_program.address(),
            cpi_accounts,
            signer_seeds,
        )
        .with_remaining_accounts(remaining);
        let _ = cpi_context.invoke(&[]);

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
pub struct Exercise {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub spectator: UncheckedAccount,
    pub proof: Account<ProofAccount>,
    pub system_program: Program<System>,
}

#[account]
pub struct ProofAccount {
    pub value: u64,
}

#[event]
pub struct ProofEvent {
    pub marker: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BoundedSequence {
    pub bytes: anchor_lang::__idl::vec::Vec<u8>,
    pub text: anchor_lang::__idl::string::String,
}
