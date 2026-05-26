use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod declared {
    use super::*;

    pub const ID: Address = Address::from_str_const("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");
}

#[derive(Clone, Copy, wincode::SchemaWrite)]
pub struct ComplexArgs {
    pub amount: u64,
    pub tag: [u8; 3],
}

#[derive(Accounts)]
pub struct Empty {}

#[derive(Accounts)]
pub struct AuthorityOnly {
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct Mixed {
    #[account(mut)]
    pub data: UncheckedAccount,
    pub authority: Signer,
    pub spectator: UncheckedAccount,
}

#[derive(Accounts)]
pub struct NestedOuter {
    pub nested: Nested<AuthorityOnly>,
    #[account(mut)]
    pub vault: UncheckedAccount,
}

#[program(interface, program_id = declared::ID)]
pub mod program {
    use super::*;

    pub fn default_disc(_ctx: &mut Context<Empty>) -> Result<()> {
        unreachable!()
    }

    #[discrim = 7]
    pub fn one_byte(_ctx: &mut Context<AuthorityOnly>, amount: u64) -> Result<()> {
        let _ = amount;
        unreachable!()
    }

    #[discrim = [1, 2, 3, 4]]
    pub fn short_disc(_ctx: &mut Context<Mixed>, flag: u8) -> Result<()> {
        let _ = flag;
        unreachable!()
    }

    #[discrim = [10, 11, 12, 13, 14, 15, 16, 17]]
    pub fn eight_byte(_ctx: &mut Context<Mixed>, amount: u64) -> Result<()> {
        let _ = amount;
        unreachable!()
    }

    #[discrim = [21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36]]
    pub fn long_disc(_ctx: &mut Context<Mixed>, args: ComplexArgs) -> Result<()> {
        let _ = args;
        unreachable!()
    }

    #[discrim = [44, 45, 46]]
    pub fn nested_accounts(_ctx: &mut Context<NestedOuter>, count: u16) -> Result<()> {
        let _ = count;
        unreachable!()
    }

    #[discrim = [50, 51, 52, 53]]
    pub fn reuse_accounts(_ctx: &mut Context<Mixed>) -> Result<()> {
        unreachable!()
    }
}

#[cfg(feature = "cpi")]
pub fn cpi_account_type_is_generated<'a>(
    accounts: cpi::accounts::NestedOuter<'a>,
) -> cpi::accounts::NestedOuter<'a> {
    accounts
}
