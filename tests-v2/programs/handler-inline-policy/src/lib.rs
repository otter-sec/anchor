use anchor_lang::prelude::*;

declare_id!("6NxceYZNn23ERJ6rDPENG8iT5bz7osPqiQeWukHaYsRs");

#[account(borsh)]
pub struct LargeState {
    pub values: [u64; 120],
}

macro_rules! large_accounts {
    ($outer:ident, $inner:ident) => {
        #[derive(Accounts)]
        pub struct $inner {
            pub account_0: BorshAccount<LargeState>,
            pub account_1: BorshAccount<LargeState>,
            pub account_2: BorshAccount<LargeState>,
            pub account_3: BorshAccount<LargeState>,
        }

        #[derive(Accounts)]
        pub struct $outer {
            pub inner_0: Nested<$inner>,
        }
    };
}

large_accounts!(LargeOne, InnerOne);
large_accounts!(LargeTwo, InnerTwo);
large_accounts!(LargeThree, InnerThree);
large_accounts!(LargeFour, InnerFour);

#[cfg(not(feature = "force-inline"))]
#[program]
pub mod handler_inline_policy {
    use super::*;

    #[handler(inline = false)]
    pub fn instruction_one(_ctx: &mut Context<LargeOne>) -> Result<()> {
        Ok(())
    }

    #[handler(inline = false)]
    pub fn instruction_two(_ctx: &mut Context<LargeTwo>) -> Result<()> {
        Ok(())
    }

    #[handler(inline = false)]
    pub fn instruction_three(_ctx: &mut Context<LargeThree>) -> Result<()> {
        Ok(())
    }

    #[handler(inline = false)]
    pub fn instruction_four(_ctx: &mut Context<LargeFour>) -> Result<()> {
        Ok(())
    }

    #[handler(inline = false)]
    pub fn instruction_five(_ctx: &mut Context<LargeOne>) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "force-inline")]
#[program]
pub mod handler_inline_policy {
    use super::*;

    pub fn instruction_one(_ctx: &mut Context<LargeOne>) -> Result<()> {
        Ok(())
    }

    pub fn instruction_two(_ctx: &mut Context<LargeTwo>) -> Result<()> {
        Ok(())
    }

    pub fn instruction_three(_ctx: &mut Context<LargeThree>) -> Result<()> {
        Ok(())
    }

    pub fn instruction_four(_ctx: &mut Context<LargeFour>) -> Result<()> {
        Ok(())
    }

    pub fn instruction_five(_ctx: &mut Context<LargeOne>) -> Result<()> {
        Ok(())
    }
}
