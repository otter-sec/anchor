use anchor_lang::prelude::*;

declare_id!("Mu1tip1eErrors11111111111111111111111111111");

#[program]
pub mod multiple_errors {
    use super::*;

    pub fn test(_ctx: Context<Test>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Test {}

#[error_code]
pub enum FirstError {
    First,
}

#[error_code]
pub enum SecondError {
    Second,
}
