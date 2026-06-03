use anchor_lang::prelude::*;

declare_id!("FdQ5d5kJDidxLP8qBm2d4G47QbDMWk6iWJ3QkYY2UAP7");

pub const CUSTOM_PROGRAM_ID: Pubkey = pubkey!("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY");
pub const NON_EXECUTABLE_ACCOUNT_ID: Pubkey =
    pubkey!("2myyNegEA6pjAHmmEsJC6JdYhW51gwxQW7ZCTWvwaKTk");
pub const CUSTOM_PROGRAM_ADDRESS: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

// Define a marker struct for our custom program ID
pub struct CustomProgramMarker;

impl Id for CustomProgramMarker {
    fn id() -> Pubkey {
        id()
    }
}

#[program]
mod custom_program {
    use super::*;

    pub fn test_program_validation(ctx: Context<TestProgramValidation>) -> Result<()> {
        // This demonstrates both types of program validation:
        // - generic_program: only validates executable (any program)
        // - system_program: validates both program ID and executable
        msg!(
            "Generic program key: {}",
            ctx.accounts.generic_program.key()
        );
        msg!("System program key: {}", ctx.accounts.system_program.key());
        msg!(
            "Custom program key: {}",
            ctx.accounts.custom_program_input.key()
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TestProgramValidation<'info> {
    /// Generic program - only validates executable (any program ID)
    pub generic_program: Program<'info>,

    /// Specific system program - validates both program ID and executable
    pub system_program: Program<'info, System>,

    /// Custom program with specific type - validates both program ID and executable  
    pub custom_program_input: Program<'info, CustomProgramMarker>,

    /// Program with an address constraint - validates both program ID and executable
    #[account(address = CUSTOM_PROGRAM_ADDRESS)]
    pub custom_program_address: Program<'info>,
}
