use crate::account::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
    #[account(init, payer = payer, space = DataAccount::LEN, constraint = payer.is_some())]
    pub optional_account: Option<Box<Account<'info, DataAccount>>>,
    pub system_program: Option<Program<'info, System>>,
    #[account(zero)]
    pub required: Account<'info, DataAccount>,
    #[account(init, seeds=[DataPda::PREFIX.as_bytes(), optional_account.as_ref().unwrap().key().as_ref()], bump, payer=payer, space=DataPda::LEN)]
    pub optional_pda: Option<Account<'info, DataPda>>,
}

#[derive(Accounts)]
#[instruction(value: u64, key: Pubkey, pda_bump: u8)]
pub struct Update<'info> {
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
    #[account(mut, seeds=[DataPda::PREFIX.as_bytes(), optional_account.as_ref().unwrap().key().as_ref()], bump = pda_bump)]
    pub optional_pda: Option<Account<'info, DataPda>>,
    #[account(mut, signer, constraint = payer.is_some())]
    pub optional_account: Option<Box<Account<'info, DataAccount>>>,
}

#[derive(Accounts)]
#[instruction(new_size: u64)]
pub struct Realloc<'info> {
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
    #[account(mut, realloc = new_size as usize, realloc::payer = payer, realloc::zero = false)]
    pub optional_pda: Option<Account<'info, DataPda>>,
    pub required: Account<'info, DataAccount>,
    pub system_program: Option<Program<'info, System>>,
    #[account(mut, signer, realloc = new_size as usize, realloc::payer = payer, realloc::zero = true)]
    pub optional_account: Option<Account<'info, DataAccount>>,
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
    #[account(mut, close = payer, has_one = data_account)]
    pub optional_pda: Option<Box<Account<'info, DataPda>>>,
    #[account(mut, signer, close = payer)]
    pub data_account: Option<Account<'info, DataAccount>>,
    pub system_program: Option<Program<'info, System>>,
}
