use anchor_lang::prelude::*;

declare_id!("8GM8KqKaxYb1jEbn5TiqqPqJYsChhjYvfpT2f6KTmUKb");

#[program]
pub mod accountloader_realloc {
    use super::*;

    /// Create the zero-copy account with the full required footprint
    /// (`DISCRIMINATOR.len() + size_of::<Data>()`).
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let mut data = ctx.accounts.data.load_init()?;
        data.value = 42;
        Ok(())
    }

    /// Shrink the `AccountLoader` to `new_len` bytes
    pub fn shrink(_ctx: Context<Shrink>, _new_len: u16) -> Result<()> {
        Ok(())
    }

    /// Verify the account stays readable after the shrink.
    pub fn read(ctx: Context<Read>) -> Result<u64> {
        let data = ctx.accounts.data.load()?;
        Ok(data.value)
    }

    /// Creates an account as an older program version left it: v1 footprint
    /// with the current discriminator. (In a real upgrade the struct name —
    /// and therefore the discriminator — doesn't change; the V1/V2 split
    /// exists only so both layouts can coexist in this test.)
    pub fn initialize_legacy(ctx: Context<InitializeLegacy>) -> Result<()> {
        let mut data = ctx.accounts.counter.try_borrow_mut_data()?;
        data[..8].copy_from_slice(CounterV2::DISCRIMINATOR);
        let v1 = CounterV1 { value: 42 };
        data[8..].copy_from_slice(bytemuck::bytes_of(&v1));
        Ok(())
    }

    /// Grows the legacy account to the v2 footprint via the `realloc`
    /// constraint, then fills the new field from existing v1 data.
    pub fn migrate(ctx: Context<Migrate>) -> Result<()> {
        let mut counter = ctx.accounts.counter.load_mut()?;
        counter.extra = counter.value * 2;
        Ok(())
    }

    pub fn read_extra(ctx: Context<ReadCounter>) -> Result<u64> {
        Ok(ctx.accounts.counter.load()?.extra)
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        seeds = [b"data"],
        bump,
        space = 8 + core::mem::size_of::<Data>(),
    )]
    pub data: AccountLoader<'info, Data>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(new_len: u16)]
pub struct Shrink<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [b"data"],
        bump,
        realloc = new_len as usize,
        realloc::payer = authority,
        realloc::zero = false,
    )]
    pub data: AccountLoader<'info, Data>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Read<'info> {
    #[account(seeds = [b"data"], bump)]
    pub data: AccountLoader<'info, Data>,
}

#[derive(Accounts)]
pub struct InitializeLegacy<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: holds the v1 layout; written manually in the handler.
    #[account(
        init,
        payer = authority,
        seeds = [b"legacy"],
        bump,
        space = 8 + core::mem::size_of::<CounterV1>(),
        owner = crate::ID,
    )]
    pub counter: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Migrate<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"legacy"],
        bump,
        realloc = 8 + core::mem::size_of::<CounterV2>(),
        realloc::payer = authority,
        realloc::zero = false,
    )]
    pub counter: AccountLoader<'info, CounterV2>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReadCounter<'info> {
    #[account(seeds = [b"legacy"], bump)]
    pub counter: AccountLoader<'info, CounterV2>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct Data {
    pub value: u64,
    pub padding: [u8; 64],
}

/// The original account layout (8-bytes body + 8-bytes discriminator = 16 bytes).
#[zero_copy]
pub struct CounterV1 {
    pub value: u64,
}

/// The upgraded layout: `extra` was added. (16 bytes body + 8 bytes discriminator = 24 bytes)
#[account(zero_copy)]
#[repr(C)]
pub struct CounterV2 {
    pub value: u64,
    pub extra: u64,
}
