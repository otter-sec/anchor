use anchor_lang::prelude::*;
declare_id!("74RTgw29JdNDxj3yCpg45y9iVjs2JmGGBNkbafSTeNv9");
#[program]
pub mod dupe {
    use super::*;
    pub fn func_one(ctx: Context<FuncOne>) -> Result<()> { Ok(()) }
    pub fn func_two(ctx: Context<FuncTwo>) -> Result<()> { Ok(()) }
}
#[derive(Accounts)]
pub struct FuncOne<'info> {
    #[account(mut)]
    /// CHECK: checked
    pub my_account: UncheckedAccount<'info>,
}
#[derive(Accounts)]
pub struct FuncTwo<'info> {
    #[account(mut)]
    pub my_account: UncheckedAccount<'info>,
}
