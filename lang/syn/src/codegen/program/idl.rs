/// Legacy IDL instruction codegen.
pub fn idl_accounts_and_functions() -> proc_macro2::TokenStream {
    use quote::quote;
    quote! {
        use anchor_lang::idl::ERASED_AUTHORITY;

        #[account("internal")]
        #[derive(Debug)]
        pub struct IdlAccount {
            // Address that can modify the IDL.
            pub authority: Pubkey,
            // Length of compressed idl bytes.
            pub data_len: u32,
            // Followed by compressed idl bytes.
        }

        impl IdlAccount {
            pub fn address(program_id: &Pubkey) -> Pubkey {
                let program_signer = Pubkey::find_program_address(&[], program_id).0;
                Pubkey::create_with_seed(&program_signer, IdlAccount::seed(), program_id)
                    .expect("Seed is always valid")
            }
            pub fn seed() -> &'static str {
                "anchor:idl"
            }
        }

        // Hacky workaround because of some internals to how account attribute
        // works. Namespaces are the root of most of the problem.
        impl anchor_lang::Owner for IdlAccount {
            fn owner() -> Pubkey {
                crate::ID
            }
        }

        // Accounts for the Create instruction.
        #[derive(Accounts)]
        pub struct IdlCreateAccounts<'info> {
            // Payer of the transaction.
            #[account(signer)]
            pub from: UncheckedAccount<'info>,
            // The deterministically defined "state" account being created via
            // `create_account_with_seed`.
            #[account(mut)]
            pub to: UncheckedAccount<'info>,
            // The program-derived-address signing off on the account creation.
            // Seeds = &[] + bump seed.
            #[account(seeds = [], bump)]
            pub base: UncheckedAccount<'info>,
            // The system program.
            pub system_program: Program<'info, System>,
            // The program whose state is being constructed.
            #[account(executable)]
            pub program: UncheckedAccount<'info>,
        }

        // Accounts for Idl instructions.
        #[derive(Accounts)]
        pub struct IdlAccounts<'info> {
            #[account(mut, has_one = authority)]
            pub idl: Account<'info, IdlAccount>,
            #[account(constraint = authority.key != &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
        }

        // Accounts for resize account instruction
        #[derive(Accounts)]
        pub struct IdlResizeAccount<'info> {
            #[account(mut, has_one = authority)]
            pub idl: Account<'info, IdlAccount>,
            #[account(mut, constraint = authority.key != &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
            pub system_program: Program<'info, System>,
        }

        // Accounts for creating an idl buffer.
        #[derive(Accounts)]
        pub struct IdlCreateBuffer<'info> {
            #[account(signer, zero)]
            pub buffer: Account<'info, IdlAccount>,
            #[account(constraint = authority.key != &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
        }

        // Accounts for upgrading the canonical IdlAccount with the buffer.
        #[derive(Accounts)]
        pub struct IdlSetBuffer<'info> {
            // The buffer with the new idl data.
            #[account(mut, constraint = buffer.authority == idl.authority)]
            pub buffer: Account<'info, IdlAccount>,
            // The idl account to be updated with the buffer's data.
            #[account(mut, has_one = authority)]
            pub idl: Account<'info, IdlAccount>,
            #[account(constraint = authority.key != &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
        }

        // Accounts for closing the canonical Idl buffer.
        #[derive(Accounts)]
        pub struct IdlCloseAccount<'info> {
            #[account(mut, has_one = authority, close = sol_destination)]
            pub account: Account<'info, IdlAccount>,
            #[account(constraint = authority.key != &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
            #[account(mut)]
            pub sol_destination: UncheckedAccount<'info>,
        }


        use std::cell::{Ref, RefMut};

        pub trait IdlTrailingData<'info> {
            fn trailing_data(self) -> Ref<'info, [u8]>;
            fn trailing_data_mut(self) -> RefMut<'info, [u8]>;
        }

        impl<'a, 'info: 'a> IdlTrailingData<'a> for &'a Account<'info, IdlAccount> {
            fn trailing_data(self) -> Ref<'a, [u8]> {
                let info: &AccountInfo<'info> = self.as_ref();
                Ref::map(info.try_borrow_data().unwrap(), |d| &d[44..])
            }
            fn trailing_data_mut(self) -> RefMut<'a, [u8]> {
                let info: &AccountInfo<'info> = self.as_ref();
                RefMut::map(info.try_borrow_mut_data().unwrap(), |d| &mut d[44..])
            }
        }


        // One time IDL account initializer. Will fail on subsequent
        // invocations.
        #[inline(never)]
        pub fn __idl_create_account(
            program_id: &Pubkey,
            accounts: &mut IdlCreateAccounts,
            data_len: u64,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlCreateAccount");

            if program_id != accounts.program.key {
                return Err(anchor_lang::error::ErrorCode::IdlInstructionInvalidProgram.into());
            }
            // Create the IDL's account.
            let from = accounts.from.key;
            let (base, nonce) = Pubkey::find_program_address(&[], program_id);
            let seed = IdlAccount::seed();
            let owner = accounts.program.key;
            let to = Pubkey::create_with_seed(&base, seed, owner).unwrap();
            // Space: account discriminator || authority pubkey || vec len || vec data
            let space = std::cmp::min(
                IdlAccount::DISCRIMINATOR.len() + 32 + 4 + data_len as usize,
                10_000
            );
            let rent = Rent::get()?;
            let lamports = rent.minimum_balance(space);
            let seeds = &[&[nonce][..]];
            let ix = anchor_lang::solana_program::system_instruction::create_account_with_seed(
                from,
                &to,
                &base,
                seed,
                lamports,
                space as u64,
                owner,
            );
            anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &[
                    accounts.from.to_account_info(),
                    accounts.to.to_account_info(),
                    accounts.base.to_account_info(),
                    accounts.system_program.to_account_info(),
                ],
                &[seeds],
            )?;

            // Deserialize the newly created account.
            let mut idl_account = {
                let mut account_data =  accounts.to.try_borrow_data()?;
                let mut account_data_slice: &[u8] = &account_data;
                IdlAccount::try_deserialize_unchecked(
                    &mut account_data_slice,
                )?
            };

            // Set the authority.
            idl_account.authority = *accounts.from.key;

            // Store the new account data.
            let mut data = accounts.to.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut cursor = std::io::Cursor::new(dst);
            idl_account.try_serialize(&mut cursor)?;

            Ok(())
        }

        #[inline(never)]
        pub fn __idl_resize_account(
            program_id: &Pubkey,
            accounts: &mut IdlResizeAccount,
            data_len: u64,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlResizeAccount");

            let data_len: usize = data_len as usize;

            // We're not going to support increasing the size of accounts that already contain data
            // because that would be messy and possibly dangerous
            if accounts.idl.data_len != 0 {
                return Err(anchor_lang::error::ErrorCode::IdlAccountNotEmpty.into());
            }

            let idl_ref = AsRef::<AccountInfo>::as_ref(&accounts.idl);
            let new_account_space = idl_ref.data_len().checked_add(std::cmp::min(
                data_len
                    .checked_sub(idl_ref.data_len())
                    .expect("data_len should always be >= the current account space"),
                10_000,
            ))
            .unwrap();

            if new_account_space > idl_ref.data_len() {
                let sysvar_rent = Rent::get()?;
                let new_rent_minimum = sysvar_rent.minimum_balance(new_account_space);
                anchor_lang::system_program::transfer(
                    anchor_lang::context::CpiContext::new(
                        accounts.system_program.key(),
                        anchor_lang::system_program::Transfer {
                            from: accounts.authority.to_account_info(),
                            to: accounts.idl.to_account_info(),
                        },
                    ),
                    new_rent_minimum
                        .checked_sub(idl_ref.lamports())
                        .unwrap(),
                )?;
                idl_ref.resize(new_account_space)?;
            }

            Ok(())

        }

        #[inline(never)]
        pub fn __idl_close_account(
            program_id: &Pubkey,
            accounts: &mut IdlCloseAccount,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlCloseAccount");

            Ok(())
        }

        #[inline(never)]
        pub fn __idl_create_buffer(
            program_id: &Pubkey,
            accounts: &mut IdlCreateBuffer,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlCreateBuffer");

            let mut buffer = &mut accounts.buffer;
            buffer.authority = *accounts.authority.key;
            Ok(())
        }

        #[inline(never)]
        pub fn __idl_write(
            program_id: &Pubkey,
            accounts: &mut IdlAccounts,
            idl_data: Vec<u8>,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlWrite");

            let prev_len: usize = ::std::convert::TryInto::<usize>::try_into(accounts.idl.data_len).unwrap();
            let new_len: usize = prev_len.checked_add(idl_data.len()).unwrap() as usize;
            accounts.idl.data_len = accounts.idl.data_len.checked_add(::std::convert::TryInto::<u32>::try_into(idl_data.len()).unwrap()).unwrap();

            use IdlTrailingData;
            let mut idl_bytes = accounts.idl.trailing_data_mut();
            let idl_expansion = &mut idl_bytes[prev_len..new_len];
            require_eq!(idl_expansion.len(), idl_data.len());
            idl_expansion.copy_from_slice(&idl_data[..]);

            Ok(())
        }

        #[inline(never)]
        pub fn __idl_set_authority(
            program_id: &Pubkey,
            accounts: &mut IdlAccounts,
            new_authority: Pubkey,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlSetAuthority");

            accounts.idl.authority = new_authority;
            Ok(())
        }

        #[inline(never)]
        pub fn __idl_set_buffer(
            program_id: &Pubkey,
            accounts: &mut IdlSetBuffer,
        ) -> anchor_lang::Result<()> {
            #[cfg(not(feature = "no-log-ix-name"))]
            anchor_lang::prelude::msg!("Instruction: IdlSetBuffer");

            accounts.idl.data_len = accounts.buffer.data_len;

            use IdlTrailingData;
            let buffer_len = ::std::convert::TryInto::<usize>::try_into(accounts.buffer.data_len).unwrap();
            let mut target = accounts.idl.trailing_data_mut();
            let source = &accounts.buffer.trailing_data()[..buffer_len];
            require_gte!(target.len(), buffer_len);
            target[..buffer_len].copy_from_slice(source);

            if target.len() > buffer_len {
                let target = &mut target[buffer_len..];
                let trailing_len = target.len();
                // SAFETY: `trailing_len` is guaranteed by the lines above
                unsafe {
                    anchor_lang::solana_program::program_memory::sol_memset(target, 0, trailing_len);
                }
            }

            Ok(())
        }
    }
}
