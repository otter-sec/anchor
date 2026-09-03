use anchor_lang::{accounts::Slab, prelude::*};

declare_id!("Spac111111111111111111111111111111111111111");

#[account]
pub struct PodState {
    pub value: u64,
}

#[account(borsh)]
pub struct BorshState {
    pub value: u64,
}

#[account]
pub struct TailHeader {
    pub value: u64,
    pub space_seen: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TailEntry {
    pub value: u64,
}

type TailAccount = Slab<TailHeader, TailEntry>;

#[program]
pub mod space_annotation {
    use super::*;

    #[discrim = 0]
    pub fn init_pod_too_small_for_discriminator(
        _ctx: &mut Context<InitPodTooSmallForDiscriminator>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn init_pod_too_small_for_min_data_len(
        _ctx: &mut Context<InitPodTooSmallForMinDataLen>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn init_pod_exact(ctx: &mut Context<InitPodExact>) -> Result<()> {
        ctx.accounts.state.value = 16;
        Ok(())
    }

    #[discrim = 3]
    pub fn init_pod_overallocated(ctx: &mut Context<InitPodOverallocated>) -> Result<()> {
        ctx.accounts.state.value = ctx.accounts.state.current_space() as u64;
        Ok(())
    }

    #[discrim = 4]
    pub fn init_borsh_too_small_for_discriminator(
        _ctx: &mut Context<InitBorshTooSmallForDiscriminator>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn init_borsh_too_small_for_payload(
        _ctx: &mut Context<InitBorshTooSmallForPayload>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 6]
    pub fn init_borsh_exact(ctx: &mut Context<InitBorshExact>) -> Result<()> {
        ctx.accounts.state.value = 16;
        Ok(())
    }

    #[discrim = 7]
    pub fn init_borsh_overallocated(ctx: &mut Context<InitBorshOverallocated>) -> Result<()> {
        ctx.accounts.state.value = 24;
        Ok(())
    }

    #[discrim = 8]
    pub fn init_tail_too_small_for_min_data_len(
        _ctx: &mut Context<InitTailTooSmallForMinDataLen>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 9]
    pub fn init_tail_exact_zero_capacity(
        ctx: &mut Context<InitTailExactZeroCapacity>,
    ) -> Result<()> {
        ctx.accounts.state.value = 32;
        ctx.accounts.state.space_seen = ctx.accounts.state.current_space() as u64;
        Ok(())
    }

    #[discrim = 10]
    pub fn init_tail_overallocated(ctx: &mut Context<InitTailOverallocated>) -> Result<()> {
        ctx.accounts.state.value = 56;
        ctx.accounts.state.space_seen = ctx.accounts.state.current_space() as u64;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitPodTooSmallForDiscriminator {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 7, seeds = [b"pod-disc"], bump)]
    pub state: Account<PodState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitPodTooSmallForMinDataLen {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 15, seeds = [b"pod-under"], bump)]
    pub state: Account<PodState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitPodExact {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 16, seeds = [b"pod-exact"], bump)]
    pub state: Account<PodState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitPodOverallocated {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 32, seeds = [b"pod-over"], bump)]
    pub state: Account<PodState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitBorshTooSmallForDiscriminator {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 7, seeds = [b"borsh-disc"], bump)]
    pub state: BorshAccount<BorshState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitBorshTooSmallForPayload {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 15, seeds = [b"borsh-payload"], bump)]
    pub state: BorshAccount<BorshState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitBorshExact {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 16, seeds = [b"borsh-exact"], bump)]
    pub state: BorshAccount<BorshState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitBorshOverallocated {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 24, seeds = [b"borsh-over"], bump)]
    pub state: BorshAccount<BorshState>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitTailTooSmallForMinDataLen {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 31, seeds = [b"tail-under"], bump)]
    pub state: TailAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitTailExactZeroCapacity {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = TailAccount::space_for(0), seeds = [b"tail-exact"], bump)]
    pub state: TailAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitTailOverallocated {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = TailAccount::space_for(3), seeds = [b"tail-over"], bump)]
    pub state: TailAccount,
    pub system_program: Program<System>,
}
