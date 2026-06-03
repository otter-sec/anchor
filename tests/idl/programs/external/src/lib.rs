use anchor_lang::prelude::*;

declare_id!("Externa1111111111111111111111111111111111111");

#[program]
pub mod external {
    use super::*;

    pub fn test_compilation(_ctx: Context<TestCompilation>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TestCompilation<'info> {
    account: Account<'info, MyAccount>,
}

#[account]
pub struct MyAccount {}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MyStruct {
    some_field: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum MyEnum {
    Unit,
    Named { name: String },
    Tuple(String),
}
