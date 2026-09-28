use {
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface},
};

declare_id!("Bench11111111111111111111111111111111111111");

macro_rules! instructions {
    ($($handler:ident: $accounts:ident),+ $(,)?) => {
        #[program]
        pub mod bench {
            use super::*;

            $(
                pub fn $handler(_ctx: Context<$accounts>) -> Result<()> {
                    Ok(())
                }
            )+
        }
    };
}

instructions! {
    account_info1: AccountInfo1,
    account_info2: AccountInfo2,
    account_info4: AccountInfo4,
    account_info8: AccountInfo8,
    account_empty_init1: AccountEmptyInit1,
    account_empty_init2: AccountEmptyInit2,
    account_empty_init4: AccountEmptyInit4,
    account_empty_init8: AccountEmptyInit8,
    account_empty1: AccountEmpty1,
    account_empty2: AccountEmpty2,
    account_empty4: AccountEmpty4,
    account_empty8: AccountEmpty8,
    account_sized_init1: AccountSizedInit1,
    account_sized_init2: AccountSizedInit2,
    account_sized_init4: AccountSizedInit4,
    account_sized_init8: AccountSizedInit8,
    account_sized1: AccountSized1,
    account_sized2: AccountSized2,
    account_sized4: AccountSized4,
    account_sized8: AccountSized8,
    account_unsized_init1: AccountUnsizedInit1,
    account_unsized_init2: AccountUnsizedInit2,
    account_unsized_init4: AccountUnsizedInit4,
    account_unsized_init8: AccountUnsizedInit8,
    account_unsized1: AccountUnsized1,
    account_unsized2: AccountUnsized2,
    account_unsized4: AccountUnsized4,
    account_unsized8: AccountUnsized8,
    boxed_account_empty_init1: BoxedAccountEmptyInit1,
    boxed_account_empty_init2: BoxedAccountEmptyInit2,
    boxed_account_empty_init4: BoxedAccountEmptyInit4,
    boxed_account_empty_init8: BoxedAccountEmptyInit8,
    boxed_account_empty1: BoxedAccountEmpty1,
    boxed_account_empty2: BoxedAccountEmpty2,
    boxed_account_empty4: BoxedAccountEmpty4,
    boxed_account_empty8: BoxedAccountEmpty8,
    boxed_account_sized_init1: BoxedAccountSizedInit1,
    boxed_account_sized_init2: BoxedAccountSizedInit2,
    boxed_account_sized_init4: BoxedAccountSizedInit4,
    boxed_account_sized_init8: BoxedAccountSizedInit8,
    boxed_account_sized1: BoxedAccountSized1,
    boxed_account_sized2: BoxedAccountSized2,
    boxed_account_sized4: BoxedAccountSized4,
    boxed_account_sized8: BoxedAccountSized8,
    boxed_account_unsized_init1: BoxedAccountUnsizedInit1,
    boxed_account_unsized_init2: BoxedAccountUnsizedInit2,
    boxed_account_unsized_init4: BoxedAccountUnsizedInit4,
    boxed_account_unsized_init8: BoxedAccountUnsizedInit8,
    boxed_account_unsized1: BoxedAccountUnsized1,
    boxed_account_unsized2: BoxedAccountUnsized2,
    boxed_account_unsized4: BoxedAccountUnsized4,
    boxed_account_unsized8: BoxedAccountUnsized8,
    boxed_interface_account_mint1: BoxedInterfaceAccountMint1,
    boxed_interface_account_mint2: BoxedInterfaceAccountMint2,
    boxed_interface_account_mint4: BoxedInterfaceAccountMint4,
    boxed_interface_account_mint8: BoxedInterfaceAccountMint8,
    boxed_interface_account_token1: BoxedInterfaceAccountToken1,
    boxed_interface_account_token2: BoxedInterfaceAccountToken2,
    boxed_interface_account_token4: BoxedInterfaceAccountToken4,
    boxed_interface_account_token8: BoxedInterfaceAccountToken8,
    interface_account_mint1: InterfaceAccountMint1,
    interface_account_mint2: InterfaceAccountMint2,
    interface_account_mint4: InterfaceAccountMint4,
    interface_account_mint8: InterfaceAccountMint8,
    interface_account_token1: InterfaceAccountToken1,
    interface_account_token2: InterfaceAccountToken2,
    interface_account_token4: InterfaceAccountToken4,
    interface1: Interface1,
    interface2: Interface2,
    interface4: Interface4,
    interface8: Interface8,
    program1: Program1,
    program2: Program2,
    program4: Program4,
    program8: Program8,
    signer1: Signer1,
    signer2: Signer2,
    signer4: Signer4,
    signer8: Signer8,
    system_account1: SystemAccount1,
    system_account2: SystemAccount2,
    system_account4: SystemAccount4,
    system_account8: SystemAccount8,
    unchecked_account1: UncheckedAccount1,
    unchecked_account2: UncheckedAccount2,
    unchecked_account4: UncheckedAccount4,
    unchecked_account8: UncheckedAccount8,
}

#[account]
pub struct Empty {}

#[account]
pub struct Sized {
    pub field: [u8; 8],
}

#[account]
pub struct Unsized {
    pub field: Vec<u8>,
}

macro_rules! accounts {
    ($name:ident, account_info; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: AccountInfo<'info>,)+ } };
    ($name:ident, account_empty; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Account<'info, Empty>,)+ } };
    ($name:ident, account_sized; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Account<'info, Sized>,)+ } };
    ($name:ident, account_unsized; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Account<'info, Unsized>,)+ } };
    ($name:ident, boxed_account_empty; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Box<Account<'info, Empty>>,)+ } };
    ($name:ident, boxed_account_sized; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Box<Account<'info, Sized>>,)+ } };
    ($name:ident, boxed_account_unsized; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Box<Account<'info, Unsized>>,)+ } };
    ($name:ident, boxed_interface_mint; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Box<InterfaceAccount<'info, Mint>>,)+ } };
    ($name:ident, boxed_interface_token; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Box<InterfaceAccount<'info, TokenAccount>>,)+ } };
    ($name:ident, interface_mint; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: InterfaceAccount<'info, Mint>,)+ } };
    ($name:ident, interface_token; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: InterfaceAccount<'info, TokenAccount>,)+ } };
    ($name:ident, interface; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Interface<'info, TokenInterface>,)+ } };
    ($name:ident, program; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Program<'info, System>,)+ } };
    ($name:ident, signer; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: Signer<'info>,)+ } };
    ($name:ident, system_account; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: SystemAccount<'info>,)+ } };
    ($name:ident, unchecked_account; $($field:ident),+) => { #[derive(Accounts)] pub struct $name<'info> { $(pub $field: UncheckedAccount<'info>,)+ } };
}

macro_rules! init_accounts {
    ($name:ident, account_empty; $($field:ident),+) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
            #[account(mut)]
            pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
            $(#[account(init, payer = payer, space = 8)] pub $field: Account<'info, Empty>,)+
        }
    };
    ($name:ident, account_sized; $($field:ident),+) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
            #[account(mut)] pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
            $(#[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())] pub $field: Account<'info, Sized>,)+
        }
    };
    ($name:ident, account_unsized; $($field:ident),+) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
            #[account(mut)] pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
            $(#[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())] pub $field: Account<'info, Unsized>,)+
        }
    };
    ($name:ident, boxed_account_empty; $($field:ident),+) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
            #[account(mut)] pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
            $(#[account(init, payer = payer, space = 8)] pub $field: Box<Account<'info, Empty>>,)+
        }
    };
    ($name:ident, boxed_account_sized; $($field:ident),+) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
            #[account(mut)] pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
            $(#[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())] pub $field: Box<Account<'info, Sized>>,)+
        }
    };
    ($name:ident, boxed_account_unsized; $($field:ident),+) => {
        #[derive(Accounts)]
        pub struct $name<'info> {
            #[account(mut)] pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
            $(#[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())] pub $field: Box<Account<'info, Unsized>>,)+
        }
    };
}

accounts!(AccountInfo1, account_info; account1);
accounts!(AccountInfo2, account_info; account1, account2);
accounts!(AccountInfo4, account_info; account1, account2, account3, account4);
accounts!(AccountInfo8, account_info; account1, account2, account3, account4, account5, account6, account7, account8);

init_accounts!(AccountEmptyInit1, account_empty; account1);
init_accounts!(AccountEmptyInit2, account_empty; account1, account2);
init_accounts!(AccountEmptyInit4, account_empty; account1, account2, account3, account4);
init_accounts!(AccountEmptyInit8, account_empty; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(AccountEmpty1, account_empty; account1);
accounts!(AccountEmpty2, account_empty; account1, account2);
accounts!(AccountEmpty4, account_empty; account1, account2, account3, account4);
accounts!(AccountEmpty8, account_empty; account1, account2, account3, account4, account5, account6, account7, account8);

init_accounts!(AccountSizedInit1, account_sized; account1);
init_accounts!(AccountSizedInit2, account_sized; account1, account2);
init_accounts!(AccountSizedInit4, account_sized; account1, account2, account3, account4);
init_accounts!(AccountSizedInit8, account_sized; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(AccountSized1, account_sized; account1);
accounts!(AccountSized2, account_sized; account1, account2);
accounts!(AccountSized4, account_sized; account1, account2, account3, account4);
accounts!(AccountSized8, account_sized; account1, account2, account3, account4, account5, account6, account7, account8);

init_accounts!(AccountUnsizedInit1, account_unsized; account1);
init_accounts!(AccountUnsizedInit2, account_unsized; account1, account2);
init_accounts!(AccountUnsizedInit4, account_unsized; account1, account2, account3, account4);
init_accounts!(AccountUnsizedInit8, account_unsized; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(AccountUnsized1, account_unsized; account1);
accounts!(AccountUnsized2, account_unsized; account1, account2);
accounts!(AccountUnsized4, account_unsized; account1, account2, account3, account4);
accounts!(AccountUnsized8, account_unsized; account1, account2, account3, account4, account5, account6, account7, account8);

init_accounts!(BoxedAccountEmptyInit1, boxed_account_empty; account1);
init_accounts!(BoxedAccountEmptyInit2, boxed_account_empty; account1, account2);
init_accounts!(BoxedAccountEmptyInit4, boxed_account_empty; account1, account2, account3, account4);
init_accounts!(BoxedAccountEmptyInit8, boxed_account_empty; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(BoxedAccountEmpty1, boxed_account_empty; account1);
accounts!(BoxedAccountEmpty2, boxed_account_empty; account1, account2);
accounts!(BoxedAccountEmpty4, boxed_account_empty; account1, account2, account3, account4);
accounts!(BoxedAccountEmpty8, boxed_account_empty; account1, account2, account3, account4, account5, account6, account7, account8);

init_accounts!(BoxedAccountSizedInit1, boxed_account_sized; account1);
init_accounts!(BoxedAccountSizedInit2, boxed_account_sized; account1, account2);
init_accounts!(BoxedAccountSizedInit4, boxed_account_sized; account1, account2, account3, account4);
init_accounts!(BoxedAccountSizedInit8, boxed_account_sized; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(BoxedAccountSized1, boxed_account_sized; account1);
accounts!(BoxedAccountSized2, boxed_account_sized; account1, account2);
accounts!(BoxedAccountSized4, boxed_account_sized; account1, account2, account3, account4);
accounts!(BoxedAccountSized8, boxed_account_sized; account1, account2, account3, account4, account5, account6, account7, account8);

init_accounts!(BoxedAccountUnsizedInit1, boxed_account_unsized; account1);
init_accounts!(BoxedAccountUnsizedInit2, boxed_account_unsized; account1, account2);
init_accounts!(BoxedAccountUnsizedInit4, boxed_account_unsized; account1, account2, account3, account4);
init_accounts!(BoxedAccountUnsizedInit8, boxed_account_unsized; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(BoxedAccountUnsized1, boxed_account_unsized; account1);
accounts!(BoxedAccountUnsized2, boxed_account_unsized; account1, account2);
accounts!(BoxedAccountUnsized4, boxed_account_unsized; account1, account2, account3, account4);
accounts!(BoxedAccountUnsized8, boxed_account_unsized; account1, account2, account3, account4, account5, account6, account7, account8);

accounts!(BoxedInterfaceAccountMint1, boxed_interface_mint; account1);
accounts!(BoxedInterfaceAccountMint2, boxed_interface_mint; account1, account2);
accounts!(BoxedInterfaceAccountMint4, boxed_interface_mint; account1, account2, account3, account4);
accounts!(BoxedInterfaceAccountMint8, boxed_interface_mint; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(BoxedInterfaceAccountToken1, boxed_interface_token; account1);
accounts!(BoxedInterfaceAccountToken2, boxed_interface_token; account1, account2);
accounts!(BoxedInterfaceAccountToken4, boxed_interface_token; account1, account2, account3, account4);
accounts!(BoxedInterfaceAccountToken8, boxed_interface_token; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(InterfaceAccountMint1, interface_mint; account1);
accounts!(InterfaceAccountMint2, interface_mint; account1, account2);
accounts!(InterfaceAccountMint4, interface_mint; account1, account2, account3, account4);
accounts!(InterfaceAccountMint8, interface_mint; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(InterfaceAccountToken1, interface_token; account1);
accounts!(InterfaceAccountToken2, interface_token; account1, account2);
accounts!(InterfaceAccountToken4, interface_token; account1, account2, account3, account4);
accounts!(Interface1, interface; account1);
accounts!(Interface2, interface; account1, account2);
accounts!(Interface4, interface; account1, account2, account3, account4);
accounts!(Interface8, interface; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(Program1, program; account1);
accounts!(Program2, program; account1, account2);
accounts!(Program4, program; account1, account2, account3, account4);
accounts!(Program8, program; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(Signer1, signer; account1);
accounts!(Signer2, signer; account1, account2);
accounts!(Signer4, signer; account1, account2, account3, account4);
accounts!(Signer8, signer; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(SystemAccount1, system_account; account1);
accounts!(SystemAccount2, system_account; account1, account2);
accounts!(SystemAccount4, system_account; account1, account2, account3, account4);
accounts!(SystemAccount8, system_account; account1, account2, account3, account4, account5, account6, account7, account8);
accounts!(UncheckedAccount1, unchecked_account; account1);
accounts!(UncheckedAccount2, unchecked_account; account1, account2);
accounts!(UncheckedAccount4, unchecked_account; account1, account2, account3, account4);
accounts!(UncheckedAccount8, unchecked_account; account1, account2, account3, account4, account5, account6, account7, account8);
