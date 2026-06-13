use crate::account::*;
use crate::program::Misc;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::token_interface::{Mint as MintInterface, TokenAccount as TokenAccountInterface};

#[derive(Accounts)]
pub struct TestTokenSeedsInit<'info> {
    #[account(
        init,
        seeds = [b"my-mint-seed".as_ref()],
        bump,
        payer = authority,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        seeds = [b"my-token-seed".as_ref()],
        bump,
        payer = authority,
        token::mint = mint,
        token::authority = authority,
    )]
    pub my_pda: Account<'info, TokenAccount>,
    #[account(mut)]
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct TestInitAssociatedToken<'info> {
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct TestInitAssociatedTokenWithTokenProgram<'info> {
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
        associated_token::token_program = associated_token_token_program,
    )]
    pub token: InterfaceAccount<'info, TokenAccountInterface>,
    pub mint: InterfaceAccount<'info, MintInterface>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: ignore
    pub associated_token_token_program: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct TestValidateAssociatedToken<'info> {
    #[account(
        associated_token::mint = mint,
        associated_token::authority = wallet,
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    /// CHECK:
    pub wallet: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(nonce: u8)]
pub struct TestInstructionConstraint<'info> {
    #[account(
        seeds = [b"my-seed", my_account.key.as_ref()],
        bump = nonce,
    )]
    /// CHECK:
    pub my_pda: UncheckedAccount<'info>,
    /// CHECK:
    pub my_account: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(domain: String, seed: Vec<u8>, bump: u8)]
pub struct TestPdaInit<'info> {
    #[account(
        init,
        seeds = [b"my-seed", domain.as_bytes(), foo.key.as_ref(), &seed],
        bump,
        payer = my_payer,
        space = DataU16::LEN + 8
    )]
    pub my_pda: Account<'info, DataU16>,
    #[account(mut)]
    pub my_payer: Signer<'info>,
    /// CHECK:
    pub foo: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestPdaInitZeroCopy<'info> {
    #[account(
        init,
        seeds = [b"my-seed".as_ref()],
        bump,
        payer = my_payer,
        space = DataZeroCopy::LEN + 8
    )]
    pub my_pda: AccountLoader<'info, DataZeroCopy>,
    #[account(mut)]
    pub my_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestPdaMutZeroCopy<'info> {
    #[account(
        mut,
        seeds = [b"my-seed".as_ref()],
        bump = my_pda.load()?.bump,
    )]
    pub my_pda: AccountLoader<'info, DataZeroCopy>,
    /// CHECK:
    pub my_payer: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Ctor {}

#[derive(Accounts)]
pub struct RemainingAccounts {}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(zero)]
    pub data: Account<'info, Data>,
}

#[derive(Accounts)]
pub struct InitializeSkipRentExempt<'info> {
    #[account(zero, rent_exempt = skip)]
    pub data: Account<'info, Data>,
}

#[derive(Accounts)]
pub struct InitializeNoRentExempt<'info> {
    /// CHECK:
    pub data: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestOwner<'info> {
    #[account(owner = *misc.key)]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
    /// CHECK:
    pub misc: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestExecutable<'info> {
    #[account(executable)]
    /// CHECK:
    pub program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestClose<'info> {
    #[account(mut, close = sol_dest)]
    pub data: Account<'info, Data>,
    /// CHECK:
    sol_dest: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestCloseTwice<'info> {
    #[account(mut, close = sol_dest)]
    pub data: Account<'info, Data>,
    /// CHECK:
    pub sol_dest: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestCloseMut<'info> {
    #[account(mut)]
    pub data: Account<'info, Data>,
    /// CHECK:
    pub sol_dest: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestSimulate {}

#[derive(Accounts)]
pub struct TestInit<'info> {
    #[account(init, payer = payer, space = DataI8::LEN + 8)]
    pub data: Account<'info, DataI8>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestInitZeroCopy<'info> {
    #[account(init, payer = payer, space = DataZeroCopy::LEN + 8)]
    pub data: AccountLoader<'info, DataZeroCopy>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestInitMint<'info> {
    #[account(init, mint::decimals = 6, mint::authority = payer, mint::freeze_authority = payer, payer = payer, )]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct TestInitMintWithTokenProgram<'info> {
    #[account(init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = payer,
        mint::freeze_authority = payer,
        mint::token_program = mint_token_program,
    )]
    pub mint: InterfaceAccount<'info, MintInterface>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: ignore
    pub mint_token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestInitToken<'info> {
    #[account(init, token::mint = mint, token::authority = payer, payer = payer, )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct TestInitTokenWithTokenProgram<'info> {
    #[account(init,
        payer = payer,
        token::mint = mint,
        token::authority = payer,
        token::token_program = token_token_program,
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: ignore
    pub token_token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestCompositePayer<'info> {
    pub composite: TestInit<'info>,
    #[account(init, payer = composite.payer, space = Data::LEN + 8)]
    pub data: Account<'info, Data>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestFetchAll<'info> {
    #[account(init, payer = authority, space = DataWithFilter::LEN + 8)]
    pub data: Account<'info, DataWithFilter>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestInitWithEmptySeeds<'info> {
    #[account(init, seeds = [], bump, payer = authority, space = Data::LEN + 8)]
    pub pda: Account<'info, Data>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestEmptySeedsConstraint<'info> {
    #[account(seeds = [], bump)]
    /// CHECK:
    pub pda: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct InitWithSpace<'info> {
    #[account(init, payer = payer, space = DataU16::LEN + 8)]
    pub data: Account<'info, DataU16>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestInitIfNeeded<'info> {
    // intentionally using more space (+500) to check whether space is checked when using init_if_needed
    #[account(init_if_needed, payer = payer, space = DataU16::LEN + 8 + 500)]
    pub data: Account<'info, DataU16>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestInitIfNeededChecksOwner<'info> {
    #[account(init_if_needed, payer = payer, space = 100, owner = *owner.key, seeds = [b"hello"], bump)]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK:
    pub owner: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(seed_data: String)]
pub struct TestInitIfNeededChecksSeeds<'info> {
    #[account(init_if_needed, payer = payer, space = 100, seeds = [seed_data.as_bytes()], bump)]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct TestInitMintIfNeeded<'info> {
    #[account(init_if_needed, mint::decimals = decimals, mint::authority = mint_authority, mint::freeze_authority = freeze_authority, payer = payer)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK:
    pub mint_authority: UncheckedAccount<'info>,
    /// CHECK:
    pub freeze_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestInitMintIfNeededWithTokenProgram<'info> {
    #[account(init_if_needed,
        payer = payer,
        mint::decimals = 6,
        mint::authority = mint_authority,
        mint::token_program = mint_token_program,
        mint::freeze_authority = freeze_authority,
    )]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: ignore
    pub mint_token_program: UncheckedAccount<'info>,
    /// CHECK: ignore
    pub mint_authority: UncheckedAccount<'info>,
    /// CHECK: ignore
    pub freeze_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestInitTokenIfNeeded<'info> {
    #[account(init_if_needed, token::mint = mint, token::authority = authority, payer = payer, )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestInitTokenIfNeededWithTokenProgram<'info> {
    #[account(init_if_needed,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_token_program
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: ignore
    pub token_token_program: UncheckedAccount<'info>,
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestInitAssociatedTokenIfNeeded<'info> {
    #[account(init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestInitAssociatedTokenIfNeededWithTokenProgram<'info> {
    #[account(init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = associated_token_token_program,
    )]
    pub token: InterfaceAccount<'info, TokenAccountInterface>,
    pub mint: InterfaceAccount<'info, MintInterface>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: ignore
    pub associated_token_token_program: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// CHECK: ignore
    pub authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestMultidimensionalArray<'info> {
    #[account(zero)]
    pub data: Account<'info, DataMultidimensionalArray>,
}

#[derive(Accounts)]
pub struct TestConstArraySize<'info> {
    #[account(zero)]
    pub data: Account<'info, DataConstArraySize>,
}

#[derive(Accounts)]
pub struct TestConstIxDataSize<'info> {
    #[account(zero)]
    pub data: Account<'info, DataConstArraySize>,
}

#[derive(Accounts)]
pub struct TestMultidimensionalArrayConstSizes<'info> {
    #[account(zero)]
    pub data: Account<'info, DataMultidimensionalArrayConstSizes>,
}

#[derive(Accounts)]
pub struct NoRentExempt<'info> {
    /// CHECK:
    pub data: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct EnforceRentExempt<'info> {
    #[account(rent_exempt = enforce)]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct InitDecreaseLamports<'info> {
    #[account(init, payer = user, space = 1000)]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitIfNeededChecksRentExemption<'info> {
    #[account(init_if_needed, payer = user, space = 1000)]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bump: u8, second_bump: u8)]
pub struct TestProgramIdConstraint<'info> {
    // not a real associated token account
    // just deriving like this for testing purposes
    #[account(seeds = [b"seed"], bump = bump, seeds::program = anchor_spl::associated_token::ID)]
    /// CHECK:
    first: UncheckedAccount<'info>,

    #[account(seeds = [b"seed"], bump = second_bump, seeds::program = crate::ID)]
    /// CHECK:
    second: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestProgramIdConstraintUsingFindPda<'info> {
    // not a real associated token account
    // just deriving like this for testing purposes
    #[account(seeds = [b"seed"], bump, seeds::program = anchor_spl::associated_token::ID)]
    /// CHECK:
    first: UncheckedAccount<'info>,

    #[account(seeds = [b"seed"], bump, seeds::program = crate::ID)]
    /// CHECK:
    second: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestUnsafeFieldSafetyErrors<'info> {
    #[doc = "test"]
    /// CHECK:
    pub data: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK:
    pub data_two: UncheckedAccount<'info>,
    #[account(
        seeds = [b"my-seed", signer.key.as_ref()],
        bump
    )]
    /// CHECK:
    pub data_three: UncheckedAccount<'info>,
    /// CHECK:
    pub data_four: UncheckedAccount<'info>,
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestConstraintToken<'info> {
    #[account(
        token::mint = mint,
        token::authority = payer
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct TestAuthorityConstraint<'info> {
    #[account(
        token::mint = mint,
        token::authority = fake_authority
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub fake_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestOnlyAuthorityConstraint<'info> {
    #[account(
        token::authority = payer
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct TestOnlyTokenProgramConstraint<'info> {
    #[account(
        token::token_program = token_token_program
    )]
    pub token: Account<'info, TokenAccount>,
    /// CHECK: ignore
    pub token_token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestOnlyMintConstraint<'info> {
    #[account(
        token::mint = mint,
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct TestMintConstraint<'info> {
    #[account(
        mint::decimals = decimals,
        mint::authority = mint_authority,
        mint::freeze_authority = freeze_authority
    )]
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub mint_authority: UncheckedAccount<'info>,
    /// CHECK: ignore
    pub freeze_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct TestMintOnlyDecimalsConstraint<'info> {
    #[account(
        mint::decimals = decimals,
    )]
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct TestMintAuthorityConstraint<'info> {
    #[account(
        mint::authority = mint_authority,
        mint::freeze_authority = freeze_authority
    )]
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub mint_authority: UncheckedAccount<'info>,
    /// CHECK: ignore
    pub freeze_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestMintOneAuthorityConstraint<'info> {
    #[account(
        mint::authority = mint_authority,
    )]
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub mint_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct TestMintMissMintAuthConstraint<'info> {
    #[account(
        mint::decimals = decimals,
        mint::freeze_authority = freeze_authority,
    )]
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub freeze_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestMintOnlyTokenProgramConstraint<'info> {
    #[account(
        mint::token_program = mint_token_program,
    )]
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub mint_token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestAssociatedToken<'info> {
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    /// CHECK: ignore
    pub authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestAssociatedTokenWithTokenProgramConstraint<'info> {
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = associated_token_token_program,
    )]
    pub token: InterfaceAccount<'info, TokenAccountInterface>,
    pub mint: InterfaceAccount<'info, MintInterface>,
    /// CHECK: ignore
    pub authority: UncheckedAccount<'info>,
    /// CHECK: ignore
    pub associated_token_token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(
    program_id: u8,
    accounts: u8,
    ix_data: u8,
    remaining_accounts: u8
)]
pub struct TestUsedIdentifiers<'info> {
    #[account(
        constraint = 1 == program_id,
        constraint = 2 == accounts,
        constraint = 3 == ix_data,
        constraint = 4 == remaining_accounts,
    )]
    /// CHECK: ignore
    pub test: UncheckedAccount<'info>,

    #[account(
        seeds = [&[program_id], &[accounts], &[ix_data], &[remaining_accounts]],
        bump = program_id,
    )]
    /// CHECK: ignore
    pub test1: UncheckedAccount<'info>,
    #[account(
        seeds = [&[program_id], &[accounts], &[ix_data], &[remaining_accounts]],
        bump = accounts,
    )]
    /// CHECK: ignore
    pub test2: UncheckedAccount<'info>,
    #[account(
        seeds = [&[program_id], &[accounts], &[ix_data], &[remaining_accounts]],
        bump = ix_data,
    )]
    /// CHECK: ignore
    pub test3: UncheckedAccount<'info>,
    #[account(
        seeds = [&[program_id], &[accounts], &[ix_data], &[remaining_accounts]],
        bump = remaining_accounts,
    )]
    /// CHECK: ignore
    pub test4: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct InitManyAssociatedTokenAccounts<'info> {
    #[account(
        init,
        payer = user,
        mint::authority = user,
        mint::decimals = 9,
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = user,
        associated_token::authority = user,
        associated_token::mint = mint,
    )]
    pub ata1: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = user,
        associated_token::authority = system_program,
        associated_token::mint = mint,
    )]
    pub ata2: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = user,
        associated_token::authority = token_program,
        associated_token::mint = mint,
    )]
    pub ata3: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = user,
        associated_token::authority = associated_token_program,
        associated_token::mint = mint,
    )]
    pub ata4: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = user,
        associated_token::authority = mint,
        associated_token::mint = mint,
    )]
    pub ata5: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct TestBoxedOwnerConstraint<'info> {
    // Redundant check to test compilation
    #[account(owner = program.key())]
    pub my_account: Box<Account<'info, Data>>,
    pub program: Program<'info, Misc>,
}

#[cfg(feature = "my-feature")]
#[derive(Accounts)]
pub struct Empty {}

#[derive(Accounts)]
pub struct TestMultipleZeroConstraint<'info> {
    #[account(zero)]
    pub one: Account<'info, Data>,
    #[account(zero, dup)] // Allow duplicate accounts to test the error
    pub two: Account<'info, Data>,
}

#[derive(Accounts)]
pub struct TestInitAndZero<'info> {
    #[account(init, payer = payer, space = Data::DISCRIMINATOR.len() + Data::LEN)]
    pub init: Account<'info, Data>,
    #[account(zero, dup)] // Allow duplicate accounts to test the error
    pub zero: Account<'info, Data>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
