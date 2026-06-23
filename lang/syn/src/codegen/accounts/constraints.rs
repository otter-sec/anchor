use {
    crate::*,
    quote::{format_ident, quote},
    std::collections::HashSet,
};

pub fn generate(f: &Field, accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let constraints = linearize(&f.constraints);

    let rent = if constraints
        .iter()
        .any(|c| matches!(c, Constraint::RentExempt(ConstraintRentExempt::Enforce)))
    {
        quote! { let __anchor_rent = Rent::get()?; }
    } else {
        quote! {}
    };

    let checks: Vec<proc_macro2::TokenStream> = constraints
        .iter()
        .map(|c| generate_constraint(f, c, accs))
        .collect();

    let mut all_checks = quote! {#(#checks)*};

    // If the field is optional we do all the inner checks as if the account
    // wasn't optional. If the account is init we also need to return an Option
    // by wrapping the resulting value with Some or returning None if it doesn't exist.
    if f.is_optional && !constraints.is_empty() {
        let ident = &f.ident;
        let ty_decl = f.ty_decl(false);
        #[allow(
            clippy::indexing_slicing,
            reason = "guarded by !constraints.is_empty() above"
        )]
        let first_constraint = &constraints[0];
        all_checks = match first_constraint {
            Constraint::Init(_) | Constraint::Zeroed(_) => {
                quote! {
                    let #ident: #ty_decl = if let Some(#ident) = #ident {
                        #all_checks
                        Some(#ident)
                    } else {
                        None
                    };
                }
            }
            _ => {
                quote! {
                    if let Some(#ident) = &#ident {
                        #all_checks
                    }
                }
            }
        };
    }

    quote! {
        #rent
        #all_checks
    }
}

pub fn generate_composite(f: &CompositeField) -> proc_macro2::TokenStream {
    let checks: Vec<proc_macro2::TokenStream> = linearize(&f.constraints)
        .iter()
        .map(|c| match c {
            Constraint::Raw(_) => c,
            #[allow(
                clippy::panic,
                reason = "invariant: linearize() only yields Raw for CompositeField; non-Raw is a \
                          bug in constraint parsing"
            )]
            _ => panic!("Invariant violation: composite constraints can only be raw or literals"),
        })
        .map(|c| generate_constraint_composite(f, c))
        .collect();
    quote! {
        #(#checks)*
    }
}

// Linearizes the constraint group so that constraints with dependencies
// run after those without.
pub fn linearize(c_group: &ConstraintGroup) -> Vec<Constraint> {
    let ConstraintGroup {
        init,
        zeroed,
        mutable,
        dup,
        signer,
        has_one,
        raw,
        owner,
        rent_exempt,
        seeds,
        executable,
        close,
        address,
        associated_token,
        token_account,
        mint,
        realloc,
    } = c_group.clone();

    let mut constraints = Vec::new();

    if let Some(c) = zeroed {
        constraints.push(Constraint::Zeroed(c));
    }
    if let Some(c) = init {
        constraints.push(Constraint::Init(c));
    }
    if let Some(c) = realloc {
        constraints.push(Constraint::Realloc(c));
    }
    if let Some(c) = seeds {
        constraints.push(Constraint::Seeds(c));
    }
    if let Some(c) = associated_token {
        constraints.push(Constraint::AssociatedToken(c));
    }
    if let Some(c) = mutable {
        constraints.push(Constraint::Mut(c));
    }
    if let Some(c) = dup {
        constraints.push(Constraint::Dup(c));
    }
    if let Some(c) = signer {
        constraints.push(Constraint::Signer(c));
    }
    constraints.append(&mut has_one.into_iter().map(Constraint::HasOne).collect());
    constraints.append(&mut raw.into_iter().map(Constraint::Raw).collect());
    if let Some(c) = owner {
        constraints.push(Constraint::Owner(c));
    }
    if let Some(c) = rent_exempt {
        constraints.push(Constraint::RentExempt(c));
    }
    if let Some(c) = executable {
        constraints.push(Constraint::Executable(c));
    }
    if let Some(c) = close {
        constraints.push(Constraint::Close(c));
    }
    if let Some(c) = address {
        constraints.push(Constraint::Address(c));
    }
    if let Some(c) = token_account {
        constraints.push(Constraint::TokenAccount(c));
    }
    if let Some(c) = mint {
        constraints.push(Constraint::Mint(c));
    }
    constraints
}

fn generate_constraint(
    f: &Field,
    c: &Constraint,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    match c {
        Constraint::Init(c) => generate_constraint_init(f, c, accs),
        Constraint::Zeroed(c) => generate_constraint_zeroed(f, c, accs),
        Constraint::Mut(c) => generate_constraint_mut(f, c),
        Constraint::Dup(_) => quote! {}, // No-op: dup is handled by duplicate checking logic
        Constraint::HasOne(c) => generate_constraint_has_one(f, c, accs),
        Constraint::Signer(c) => generate_constraint_signer(f, c),
        Constraint::Raw(c) => generate_constraint_raw(&f.ident, c),
        Constraint::Owner(c) => generate_constraint_owner(f, c),
        Constraint::RentExempt(c) => generate_constraint_rent_exempt(f, c),
        Constraint::Seeds(c) => generate_constraint_seeds(f, c),
        Constraint::Executable(c) => generate_constraint_executable(f, c),
        Constraint::Close(c) => generate_constraint_close(f, c, accs),
        Constraint::Address(c) => generate_constraint_address(f, c),
        Constraint::AssociatedToken(c) => generate_constraint_associated_token(f, c, accs),
        Constraint::TokenAccount(c) => generate_constraint_token_account(f, c, accs),
        Constraint::Mint(c) => generate_constraint_mint(f, c, accs),
        Constraint::Realloc(c) => generate_constraint_realloc(f, c, accs),
    }
}

fn generate_constraint_composite(f: &CompositeField, c: &Constraint) -> proc_macro2::TokenStream {
    match c {
        Constraint::Raw(c) => generate_constraint_raw(&f.ident, c),
        #[allow(
            clippy::panic,
            reason = "invariant: only Raw constraints reach generate_constraint_composite; \
                      non-Raw is a bug in codegen dispatch"
        )]
        _ => panic!("Invariant violation"),
    }
}

fn generate_constraint_address(f: &Field, c: &ConstraintAddress) -> proc_macro2::TokenStream {
    let field = &f.ident;
    let addr = &c.address;
    let error = generate_custom_error(
        field,
        &c.error,
        quote! { ConstraintAddress },
        &Some(&(quote! { actual }, quote! { expected })),
    );
    quote! {
        {
            let actual = #field.key();
            let expected = #addr;
            if actual != expected {
                return #error;
            }
        }
    }
}

pub fn generate_constraint_init(
    f: &Field,
    c: &ConstraintInitGroup,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    generate_constraint_init_group(f, c, accs)
}

pub fn generate_constraint_zeroed(
    f: &Field,
    _c: &ConstraintZeroed,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let account_ty = f.account_ty();
    let discriminator = quote! { #account_ty::DISCRIMINATOR };

    let field = &f.ident;
    let name_str = field.to_string();
    let ty_decl = f.ty_decl(true);
    let from_account_info = f.from_account_info(None, false);

    // Require `zero` constraint accounts to be unique by:
    //
    // 1. Getting the names of all accounts that have the `zero` or the `init` constraints and are
    //    declared before the current field (in order to avoid checking the same field).
    // 2. Comparing the key of the current field with all the previous fields' keys.
    // 3. Returning an error if a match is found.
    let unique_account_checks = accs
        .fields
        .iter()
        .filter_map(|af| match af {
            AccountField::Field(field) => Some(field),
            _ => None,
        })
        .take_while(|field| field.ident != f.ident)
        .filter(|field| field.constraints.is_zeroed() || field.constraints.init.is_some())
        .map(|other_field| {
            let other = &other_field.ident;
            let err = quote! {
                Err(
                    anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintZero
                    ).with_account_name(#name_str)
                )
            };
            if other_field.is_optional {
                quote! {
                    if #other.is_some() && #field.key == &#other.as_ref().unwrap().key() {
                        return #err;
                    }
                }
            } else {
                quote! {
                    if #field.key == &#other.key() {
                        return #err;
                    }
                }
            }
        });

    quote! {
        let #field: #ty_decl = {
            let mut __data: &[u8] = &#field.try_borrow_data()?;
            if __data.len() < #discriminator.len() {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountDidNotDeserialize).with_account_name(#name_str));
            }
            let __disc = &__data[..#discriminator.len()];
            let __has_disc = __disc.iter().any(|b| *b != 0);
            if __has_disc {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintZero).with_account_name(#name_str));
            }
            #(#unique_account_checks)*
            #from_account_info
        };
    }
}

pub fn generate_constraint_close(
    f: &Field,
    c: &ConstraintClose,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let field = &f.ident;
    let name_str = field.to_string();
    let target = &c.sol_dest;
    let target_optional_check =
        OptionalCheckScope::new_with_field(accs, field).generate_check(target);
    quote! {
        {
            #target_optional_check
            if #field.key() == #target.key() {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintClose).with_account_name(#name_str));
            }
        }
    }
}

pub fn generate_constraint_mut(f: &Field, c: &ConstraintMut) -> proc_macro2::TokenStream {
    let ident = &f.ident;
    let account_ref = generate_account_ref(f);
    let error = generate_custom_error(ident, &c.error, quote! { ConstraintMut }, &None);
    quote! {
        if !#account_ref.is_writable {
            return #error;
        }
    }
}

pub fn generate_constraint_has_one(
    f: &Field,
    c: &ConstraintHasOne,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let target = &c.join_target;
    let ident = &f.ident;
    let field = match &f.ty {
        Ty::AccountLoader(_) => quote! {#ident.load()?},
        _ => quote! {#ident},
    };
    let my_key = match &f.ty {
        Ty::LazyAccount(_) => {
            let load_ident = format_ident!("load_{}", target.to_token_stream().to_string());
            quote! { *#field.#load_ident()? }
        }
        _ => quote! { #field.#target },
    };
    let error = generate_custom_error(
        ident,
        &c.error,
        quote! { ConstraintHasOne },
        &Some(&(quote! { my_key }, quote! { target_key })),
    );
    let target_optional_check =
        OptionalCheckScope::new_with_field(accs, &field).generate_check(target);

    quote! {
        {
            #target_optional_check
            let my_key = #my_key;
            let target_key = #target.key();
            if my_key != target_key {
                return #error;
            }
        }
    }
}

pub fn generate_constraint_signer(f: &Field, c: &ConstraintSigner) -> proc_macro2::TokenStream {
    let ident = &f.ident;
    let account_ref = generate_account_ref(f);

    let error = generate_custom_error(ident, &c.error, quote! { ConstraintSigner }, &None);
    quote! {
        if !#account_ref.is_signer {
            return #error;
        }
    }
}

pub fn generate_constraint_raw(ident: &Ident, c: &ConstraintRaw) -> proc_macro2::TokenStream {
    let raw = &c.raw;
    let error = generate_custom_error(ident, &c.error, quote! { ConstraintRaw }, &None);
    quote! {
        if !(#raw) {
            return #error;
        }
    }
}

pub fn generate_constraint_owner(f: &Field, c: &ConstraintOwner) -> proc_macro2::TokenStream {
    let ident = &f.ident;
    let maybe_deref = if match &f.ty {
        Ty::Account(AccountTy { boxed, .. })
        | Ty::InterfaceAccount(InterfaceAccountTy { boxed, .. }) => *boxed,
        _ => false,
    } {
        quote!(*)
    } else {
        Default::default()
    };
    let owner_address = &c.owner_address;
    let error = generate_custom_error(
        ident,
        &c.error,
        quote! { ConstraintOwner },
        &Some(&(quote! { *my_owner }, quote! { owner_address })),
    );

    quote! {
        {
            let my_owner = AsRef::<AccountInfo>::as_ref(& #maybe_deref #ident).owner;
            let owner_address = #owner_address;
            if my_owner != &owner_address {
                return #error;
            }
        }
    }
}

pub fn generate_constraint_rent_exempt(
    f: &Field,
    c: &ConstraintRentExempt,
) -> proc_macro2::TokenStream {
    let ident = &f.ident;
    let name_str = ident.to_string();
    let info = quote! {
        #ident.to_account_info()
    };
    match c {
        ConstraintRentExempt::Skip => quote! {},
        ConstraintRentExempt::Enforce => quote! {
            if !__anchor_rent.is_exempt(#info.lamports(), #info.try_data_len()?) {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintRentExempt).with_account_name(#name_str));
            }
        },
    }
}

fn generate_constraint_realloc(
    f: &Field,
    c: &ConstraintReallocGroup,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let field = &f.ident;
    let account_name = field.to_string();
    let new_space = &c.space;
    let payer = &c.payer;

    let mut optional_check_scope = OptionalCheckScope::new_with_field(accs, field);
    let payer_optional_check = optional_check_scope.generate_check(payer);
    let system_program_optional_check =
        optional_check_scope.generate_check(quote! {system_program});

    quote! {
        // Blocks duplicate account reallocs in a single instruction to prevent accidental account overwrites
        // and to ensure the calculation of the change in bytes is based on account size at program entry
        // which inheritantly guarantee idempotency.
        if __reallocs.contains(&#field.key()) {
            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountDuplicateReallocs).with_account_name(#account_name));
        }

        let __anchor_rent = anchor_lang::prelude::Rent::get()?;
        let __field_info = #field.to_account_info();
        let __new_rent_minimum = __anchor_rent.minimum_balance(#new_space);

        let __delta_space = (::std::convert::TryInto::<isize>::try_into(#new_space).unwrap())
            .checked_sub(::std::convert::TryInto::try_into(__field_info.data_len()).unwrap())
            .unwrap();

        if __delta_space != 0 {
            #payer_optional_check
            if __delta_space > 0 {
                #system_program_optional_check
                if ::std::convert::TryInto::<usize>::try_into(__delta_space).unwrap() > anchor_lang::solana_program::entrypoint::MAX_PERMITTED_DATA_INCREASE {
                    return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountReallocExceedsLimit).with_account_name(#account_name));
                }

                if __new_rent_minimum > __field_info.lamports() {
                    anchor_lang::system_program::transfer(
                        anchor_lang::context::CpiContext::new(
                            system_program.key(),
                            anchor_lang::system_program::Transfer {
                                from: #payer.to_account_info(),
                                to: __field_info.clone(),
                            },
                        ),
                        __new_rent_minimum.checked_sub(__field_info.lamports()).unwrap(),
                    )?;
                }
            } else {
                let __lamport_amt = __field_info.lamports().checked_sub(__new_rent_minimum).unwrap();
                anchor_lang::Lamports::add_lamports(&#payer, __lamport_amt)?;
                anchor_lang::Lamports::sub_lamports(&__field_info, __lamport_amt)?;
            }

            __field_info.resize(#new_space)?;
            __reallocs.insert(#field.key());
        }
    }
}

fn generate_constraint_init_group(
    f: &Field,
    c: &ConstraintInitGroup,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let field = &f.ident;
    let name_str = f.ident.to_string();
    let ty_decl = f.ty_decl(true);
    let if_needed = if c.if_needed {
        quote! {true}
    } else {
        quote! {false}
    };
    let space = &c.space;
    let payer = &c.payer;

    // Convert from account info to account context wrapper type.
    let from_account_info = f.from_account_info(Some(&c.kind), true);
    let from_account_info_unchecked = f.from_account_info(Some(&c.kind), false);

    let account_ref = generate_account_ref(f);

    // PDA bump seeds.
    let (find_pda, seeds_with_bump) = match &c.seeds {
        None => (quote! {}, quote! {}),
        Some(c) => match &c.seeds {
            // If the bump is provided with init *and target*, then force it to be the
            // canonical bump.
            //
            // Note that for `#[account(init, seeds)]`, find_program_address has already
            // been run in the init constraint find_pda variable.
            SeedsExpr::List(list) => {
                // Optional prefix (either empty or "<list>,")
                let maybe_seeds_plus_comma = (!list.is_empty()).then(|| quote! { #list, });

                let validate_pda = if let Some(b) = &c.bump {
                    quote! {
                        if #field.key() != __pda_address {
                            return Err(anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintSeeds
                            ).with_account_name(#name_str)
                             .with_pubkeys((#field.key(), __pda_address)));
                        }
                        if __bump != #b {
                            return Err(anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintSeeds
                            ).with_account_name(#name_str)
                             .with_values((__bump, #b)));
                        }
                    }
                } else {
                    quote! {
                        if #field.key() != __pda_address {
                            return Err(anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintSeeds
                            ).with_account_name(#name_str)
                             .with_pubkeys((#field.key(), __pda_address)));
                        }
                    }
                };

                let bump_tok = if f.is_optional {
                    quote!(Some(__bump))
                } else {
                    quote!(__bump)
                };

                (
                    quote! {
                        let (__pda_address, __bump) = Pubkey::find_program_address(
                            &[ #maybe_seeds_plus_comma ],
                            __program_id,
                        );
                        __bumps.#field = #bump_tok;
                        #validate_pda
                    },
                    quote! {
                        &[
                            #maybe_seeds_plus_comma
                            &[__bump][..]
                        ][..]
                    },
                )
            }
            SeedsExpr::Expr(expr) => {
                let bump_tok = if f.is_optional {
                    quote!(Some(__bump))
                } else {
                    quote!(__bump)
                };

                (
                    quote! {
                        let __seeds_slice: &[&[u8]] = #expr;
                        let (__pda_address, __bump) =
                            Pubkey::find_program_address(__seeds_slice, __program_id);
                        __bumps.#field = #bump_tok;

                        // Build signer seeds at runtime = seeds + bump
                        let __bump_bytes: [u8; 1] = [__bump];
                        let mut __signer_seeds_vec: ::std::vec::Vec<&[u8]> = __seeds_slice.to_vec();
                        __signer_seeds_vec.push(&__bump_bytes[..]);
                        let __signer_seeds = __signer_seeds_vec;

                        if #field.key() != __pda_address {
                            return Err(anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintSeeds
                            ).with_account_name(#name_str)
                             .with_pubkeys((#field.key(), __pda_address)));
                        }
                    },
                    quote! { &__signer_seeds[..] },
                )
            }
        },
    };

    // Optional check idents
    let system_program = &quote! {system_program};
    let associated_token_program = &quote! {associated_token_program};
    let rent = &quote! {rent};

    let mut check_scope = OptionalCheckScope::new_with_field(accs, field);
    match &c.kind {
        InitKind::Token {
            owner,
            mint,
            token_program,
        } => {
            let token_program = match token_program {
                Some(t) => t.to_token_stream(),
                None => quote! {token_program},
            };

            let owner_optional_check = check_scope.generate_check(owner);
            let mint_optional_check = check_scope.generate_check(mint);

            let system_program_optional_check = check_scope.generate_check(system_program);
            let token_program_optional_check = check_scope.generate_check(&token_program);
            let rent_optional_check = check_scope.generate_check(rent);

            let optional_checks = quote! {
                #system_program_optional_check
                #token_program_optional_check
                #rent_optional_check
                #owner_optional_check
                #mint_optional_check
            };

            let payer_optional_check = check_scope.generate_check(payer);

            let token_account_space = generate_get_token_account_space(mint);

            let create_account_or_fund_allocate_assign =
                generate_create_account_or_fund_allocate_assign(
                    field,
                    quote! {#token_account_space},
                    quote! {&#token_program.key()},
                    quote! {#payer},
                    seeds_with_bump,
                );

            quote! {
                // Define the bump and pda variable.
                #find_pda

                let #field: #ty_decl = ({ #[inline(never)] || {
                    // Checks that all the required accounts for this operation are present.
                    #optional_checks

                    let owner_program = #account_ref.owner;
                    if !#if_needed || owner_program == &anchor_lang::solana_program::system_program::ID {
                        #payer_optional_check

                        // Create the account with the system program.
                        #create_account_or_fund_allocate_assign

                        // Initialize the token account.
                        let cpi_program_id = #token_program.key();
                        let accounts = ::anchor_spl::token_interface::InitializeAccount3 {
                            account: #field.to_account_info(),
                            mint: #mint.to_account_info(),
                            authority: #owner.to_account_info(),
                        };
                        let cpi_ctx = anchor_lang::context::CpiContext::new(cpi_program_id, accounts);
                        ::anchor_spl::token_interface::initialize_account3(cpi_ctx)?;
                    }

                    let pa: #ty_decl = #from_account_info_unchecked;
                    if #if_needed {
                        if pa.mint != #mint.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintTokenMint).with_account_name(#name_str).with_pubkeys((pa.mint, #mint.key())));
                        }
                        if pa.owner != #owner.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintTokenOwner).with_account_name(#name_str).with_pubkeys((pa.owner, #owner.key())));
                        }
                        if owner_program != &#token_program.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintTokenTokenProgram).with_account_name(#name_str).with_pubkeys((*owner_program, #token_program.key())));
                        }
                    }
                    Ok(pa)
                }})()?;
            }
        }
        InitKind::AssociatedToken {
            owner,
            mint,
            token_program,
        } => {
            let token_program = match token_program {
                Some(t) => t.to_token_stream(),
                None => quote! {token_program},
            };
            let owner_optional_check = check_scope.generate_check(owner);
            let mint_optional_check = check_scope.generate_check(mint);

            let system_program_optional_check = check_scope.generate_check(system_program);
            let token_program_optional_check = check_scope.generate_check(&token_program);
            let associated_token_program_optional_check =
                check_scope.generate_check(associated_token_program);
            let rent_optional_check = check_scope.generate_check(rent);

            let optional_checks = quote! {
                #system_program_optional_check
                #token_program_optional_check
                #associated_token_program_optional_check
                #rent_optional_check
                #owner_optional_check
                #mint_optional_check
            };

            let payer_optional_check = check_scope.generate_check(payer);

            quote! {
                // Define the bump and pda variable.
                #find_pda

                let #field: #ty_decl = ({ #[inline(never)] || {
                    // Checks that all the required accounts for this operation are present.
                    #optional_checks

                    let owner_program = #account_ref.owner;
                    if !#if_needed || owner_program == &anchor_lang::solana_program::system_program::ID {
                        #payer_optional_check

                        ::anchor_spl::associated_token::create(
                            anchor_lang::context::CpiContext::new(
                                associated_token_program.key(),
                                ::anchor_spl::associated_token::Create {
                                    payer: #payer.to_account_info(),
                                    associated_token: #field.to_account_info(),
                                    authority: #owner.to_account_info(),
                                    mint: #mint.to_account_info(),
                                    system_program: system_program.to_account_info(),
                                    token_program: #token_program.to_account_info(),
                                }
                            )
                        )?;
                    }
                    let pa: #ty_decl = #from_account_info_unchecked;
                    if #if_needed {
                        if pa.mint != #mint.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintTokenMint).with_account_name(#name_str).with_pubkeys((pa.mint, #mint.key())));
                        }
                        if pa.owner != #owner.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintTokenOwner).with_account_name(#name_str).with_pubkeys((pa.owner, #owner.key())));
                        }
                        if owner_program != &#token_program.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintAssociatedTokenTokenProgram).with_account_name(#name_str).with_pubkeys((*owner_program, #token_program.key())));
                        }

                        if pa.key() != ::anchor_spl::associated_token::get_associated_token_address_with_program_id(&#owner.key(), &#mint.key(), &#token_program.key()) {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountNotAssociatedTokenAccount).with_account_name(#name_str));
                        }
                    }
                    Ok(pa)
                }})()?;
            }
        }
        InitKind::Mint {
            owner,
            decimals,
            freeze_authority,
            token_program,
            group_pointer_authority,
            group_pointer_group_address,
            group_member_pointer_authority,
            group_member_pointer_member_address,
            metadata_pointer_authority,
            metadata_pointer_metadata_address,
            close_authority,
            permanent_delegate,
            transfer_hook_authority,
            transfer_hook_program_id,
            pausable_authority,
        } => {
            let token_program = match token_program {
                Some(t) => t.to_token_stream(),
                None => quote! {token_program},
            };
            let owner_optional_check = check_scope.generate_check(owner);
            let freeze_authority_optional_check = match freeze_authority {
                Some(fa) => check_scope.generate_check(fa),
                None => quote! {},
            };

            // extension checks

            let group_pointer_authority_check = match group_pointer_authority {
                Some(gpa) => check_scope.generate_check(gpa),
                None => quote! {},
            };

            let group_pointer_group_address_check = match group_pointer_group_address {
                Some(gpga) => check_scope.generate_check(gpga),
                None => quote! {},
            };

            let group_member_pointer_authority_check = match group_member_pointer_authority {
                Some(gmpa) => check_scope.generate_check(gmpa),
                None => quote! {},
            };

            let group_member_pointer_member_address_check =
                match group_member_pointer_member_address {
                    Some(gmpm) => check_scope.generate_check(gmpm),
                    None => quote! {},
                };

            let metadata_pointer_authority_check = match metadata_pointer_authority {
                Some(mpa) => check_scope.generate_check(mpa),
                None => quote! {},
            };

            let metadata_pointer_metadata_address_check = match metadata_pointer_metadata_address {
                Some(mpma) => check_scope.generate_check(mpma),
                None => quote! {},
            };

            let close_authority_check = match close_authority {
                Some(ca) => check_scope.generate_check(ca),
                None => quote! {},
            };

            let transfer_hook_authority_check = match transfer_hook_authority {
                Some(tha) => check_scope.generate_check(tha),
                None => quote! {},
            };

            let transfer_hook_program_id_check = match transfer_hook_program_id {
                Some(thpid) => check_scope.generate_check(thpid),
                None => quote! {},
            };

            let permanent_delegate_check = match permanent_delegate {
                Some(pd) => check_scope.generate_check(pd),
                None => quote! {},
            };

            let pausable_authority_check = match pausable_authority {
                Some(pac) => check_scope.generate_check(pac),
                None => quote! {},
            };

            let system_program_optional_check = check_scope.generate_check(system_program);
            let token_program_optional_check = check_scope.generate_check(&token_program);
            let rent_optional_check = check_scope.generate_check(rent);

            let optional_checks = quote! {
                #system_program_optional_check
                #token_program_optional_check
                #rent_optional_check
                #owner_optional_check
                #freeze_authority_optional_check
                #group_pointer_authority_check
                #group_pointer_group_address_check
                #group_member_pointer_authority_check
                #group_member_pointer_member_address_check
                #metadata_pointer_authority_check
                #metadata_pointer_metadata_address_check
                #close_authority_check
                #transfer_hook_authority_check
                #transfer_hook_program_id_check
                #permanent_delegate_check
                #pausable_authority_check
            };

            let payer_optional_check = check_scope.generate_check(payer);

            let mut extensions = vec![];
            if group_pointer_authority.is_some() || group_pointer_group_address.is_some() {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::GroupPointer});
            }

            if group_member_pointer_authority.is_some()
                || group_member_pointer_member_address.is_some()
            {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::GroupMemberPointer});
            }

            if metadata_pointer_authority.is_some() || metadata_pointer_metadata_address.is_some() {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::MetadataPointer});
            }

            if close_authority.is_some() {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::MintCloseAuthority});
            }

            if transfer_hook_authority.is_some() || transfer_hook_program_id.is_some() {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::TransferHook});
            }

            if permanent_delegate.is_some() {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::PermanentDelegate});
            }

            if pausable_authority.is_some() {
                extensions.push(quote! {::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::Pausable})
            }

            let mint_space = if extensions.is_empty() {
                quote! { ::anchor_spl::token::Mint::LEN }
            } else {
                quote! { ::anchor_spl::token_interface::find_mint_account_size(Some(&vec![#(#extensions),*]))? }
            };

            let extensions = if extensions.is_empty() {
                quote! {Option::<&::anchor_spl::token_interface::ExtensionsVec>::None}
            } else {
                quote! {Option::<&::anchor_spl::token_interface::ExtensionsVec>::Some(&vec![#(#extensions),*])}
            };

            let freeze_authority = match freeze_authority {
                Some(fa) => quote! { Option::<&anchor_lang::prelude::Pubkey>::Some(&#fa.key()) },
                None => quote! { Option::<&anchor_lang::prelude::Pubkey>::None },
            };

            let group_pointer_authority = match group_pointer_authority {
                Some(gpa) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#gpa.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let group_pointer_group_address = match group_pointer_group_address {
                Some(gpga) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#gpga.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let group_member_pointer_authority = match group_member_pointer_authority {
                Some(gmpa) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#gmpa.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let group_member_pointer_member_address = match group_member_pointer_member_address {
                Some(gmpma) => {
                    quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#gmpma.key()) }
                }
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let metadata_pointer_authority = match metadata_pointer_authority {
                Some(mpa) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#mpa.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let metadata_pointer_metadata_address = match metadata_pointer_metadata_address {
                Some(mpma) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#mpma.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let close_authority = match close_authority {
                Some(ca) => quote! { Option::<&anchor_lang::prelude::Pubkey>::Some(&#ca.key()) },
                None => quote! { Option::<&anchor_lang::prelude::Pubkey>::None },
            };

            let permanent_delegate = match permanent_delegate {
                Some(pd) => quote! { Option::<&anchor_lang::prelude::Pubkey>::Some(&#pd.key()) },
                None => quote! { Option::<&anchor_lang::prelude::Pubkey>::None },
            };

            let transfer_hook_authority = match transfer_hook_authority {
                Some(tha) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#tha.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let transfer_hook_program_id = match transfer_hook_program_id {
                Some(thpid) => {
                    quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#thpid.key()) }
                }
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let pausable_authority = match pausable_authority {
                Some(pa) => quote! { Option::<anchor_lang::prelude::Pubkey>::Some(#pa.key()) },
                None => quote! { Option::<anchor_lang::prelude::Pubkey>::None },
            };

            let create_account_or_fund_allocate_assign =
                generate_create_account_or_fund_allocate_assign(
                    field,
                    mint_space,
                    quote! {&#token_program.key()},
                    quote! {#payer},
                    seeds_with_bump,
                );

            quote! {
                // Define the bump and pda variable.
                #find_pda

                let #field: #ty_decl = ({ #[inline(never)] || {
                    // Checks that all the required accounts for this operation are present.
                    #optional_checks

                    let owner_program = AsRef::<AccountInfo>::as_ref(&#field).owner;
                    if !#if_needed || owner_program == &anchor_lang::solana_program::system_program::ID {
                        // Define payer variable.
                        #payer_optional_check

                        // Create the account with the system program.
                        #create_account_or_fund_allocate_assign

                        let cpi_program_id = #token_program.key();

                        // Initialize extensions.
                        if let Some(extensions) = #extensions {

                            for e in extensions {
                                match e {
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::GroupPointer => {
                                        ::anchor_spl::token_interface::group_pointer_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::GroupPointerInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #group_pointer_authority, #group_pointer_group_address)?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::GroupMemberPointer => {
                                        ::anchor_spl::token_interface::group_member_pointer_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::GroupMemberPointerInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #group_member_pointer_authority, #group_member_pointer_member_address)?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::MetadataPointer => {
                                        ::anchor_spl::token_interface::metadata_pointer_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::MetadataPointerInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #metadata_pointer_authority, #metadata_pointer_metadata_address)?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::MintCloseAuthority => {
                                        ::anchor_spl::token_interface::mint_close_authority_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::MintCloseAuthorityInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #close_authority)?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::TransferHook => {
                                        ::anchor_spl::token_interface::transfer_hook_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::TransferHookInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #transfer_hook_authority, #transfer_hook_program_id)?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::NonTransferable => {
                                        ::anchor_spl::token_interface::non_transferable_mint_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::NonTransferableMintInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }))?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::PermanentDelegate => {
                                        ::anchor_spl::token_interface::permanent_delegate_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::PermanentDelegateInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #permanent_delegate.unwrap())?;
                                    },
                                    ::anchor_spl::token_interface::spl_token_2022::extension::ExtensionType::Pausable => {
                                        ::anchor_spl::token_interface::pausable_initialize(anchor_lang::context::CpiContext::new(cpi_program_id, ::anchor_spl::token_interface::PausableInitialize {
                                            token_program_id: #token_program.to_account_info(),
                                            mint: #field.to_account_info(),
                                        }), #pausable_authority.unwrap())?;
                                    },
                                    // All extensions specified by the user should be implemented.
                                    // If this line runs, it means there is a bug in the codegen.
                                    _ => unimplemented!("{e:?}"),
                                }
                            };
                        }

                        // Initialize the mint account.
                        let accounts = ::anchor_spl::token_interface::InitializeMint2 {
                            mint: #field.to_account_info(),
                        };
                        let cpi_ctx = anchor_lang::context::CpiContext::new(cpi_program_id, accounts);
                        ::anchor_spl::token_interface::initialize_mint2(cpi_ctx, #decimals, &#owner.key(), #freeze_authority)?;
                    }

                    let pa: #ty_decl = #from_account_info_unchecked;
                    if #if_needed {
                        if pa.mint_authority != anchor_lang::solana_program::program_option::COption::Some(#owner.key()) {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMintMintAuthority).with_account_name(#name_str));
                        }
                        if pa.freeze_authority
                            .as_ref()
                            .map(|fa| #freeze_authority.as_ref().map(|expected_fa| fa != *expected_fa).unwrap_or(true))
                            .unwrap_or(#freeze_authority.is_some()) {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMintFreezeAuthority).with_account_name(#name_str));
                        }
                        if pa.decimals != #decimals {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMintDecimals).with_account_name(#name_str).with_values((pa.decimals, #decimals)));
                        }
                        if owner_program != &#token_program.key() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintMintTokenProgram).with_account_name(#name_str).with_pubkeys((*owner_program, #token_program.key())));
                        }
                    }
                    Ok(pa)
                }})()?;
            }
        }
        InitKind::Program { owner } | InitKind::Interface { owner } => {
            // Define the space variable.
            let space = quote! {let space = #space;};

            let system_program_optional_check = check_scope.generate_check(system_program);

            // Define the owner of the account being created. If not specified,
            // default to the currently executing program.
            let (owner, owner_optional_check) = match owner {
                None => (
                    quote! {
                        __program_id
                    },
                    quote! {},
                ),

                Some(o) => {
                    // We clone the `check_scope` here to avoid collisions with the
                    // `payer_optional_check`, which is in a separate scope
                    let owner_optional_check = check_scope.clone().generate_check(o);
                    (
                        quote! {
                            &#o
                        },
                        owner_optional_check,
                    )
                }
            };

            let payer_optional_check = check_scope.generate_check(payer);

            let optional_checks = quote! {
                #system_program_optional_check
            };

            // CPI to the system program to create the account.
            let create_account_or_fund_allocate_assign =
                generate_create_account_or_fund_allocate_assign(
                    field,
                    quote! {space},
                    owner.clone(),
                    quote! {#payer},
                    seeds_with_bump,
                );

            // Put it all together.
            quote! {
                // Define the bump variable.
                #find_pda

                let #field = ({ #[inline(never)] || {
                    // Checks that all the required accounts for this operation are present.
                    #optional_checks

                    let actual_field = #account_ref;
                    let actual_owner = actual_field.owner;

                    // Define the account space variable.
                    #space

                    // Create the account. Always do this in the event
                    // if needed is not specified or the system program is the owner.
                    let pa: #ty_decl = if !#if_needed || actual_owner == &anchor_lang::solana_program::system_program::ID {
                        #payer_optional_check

                        // CPI to the system program to create.
                        #create_account_or_fund_allocate_assign

                        // Convert from account info to account context wrapper type.
                        #from_account_info_unchecked
                    } else {
                        // Convert from account info to account context wrapper type.
                        #from_account_info
                    };

                    // Assert the account was created correctly.
                    if #if_needed {
                        #owner_optional_check
                        if space != actual_field.data_len() {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSpace).with_account_name(#name_str).with_values((space, actual_field.data_len())));
                        }

                        if actual_owner != #owner {
                            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintOwner).with_account_name(#name_str).with_pubkeys((*actual_owner, *#owner)));
                        }

                        {
                            let required_lamports = __anchor_rent.minimum_balance(space);
                            if anchor_lang::Lamports::get_lamports(actual_field) < required_lamports {
                                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintRentExempt).with_account_name(#name_str));
                            }
                        }
                    }

                    // Done.
                    Ok(pa)
                }})()?;
            }
        }
    }
}

fn generate_constraint_seeds(f: &Field, c: &ConstraintSeedsGroup) -> proc_macro2::TokenStream {
    if c.is_init {
        // Note that for `#[account(init, seeds)]`, the seed generation and checks is checked in
        // the init constraint find_pda/validate_pda block, so we don't do anything here and
        // return nothing!
        quote! {}
    } else {
        let name = &f.ident;
        let name_str = name.to_string();

        let deriving_program_id = c
            .program_seed
            .clone()
            // If they specified a seeds::program to use when deriving the PDA, use it.
            .map(|program_id| quote! { #program_id.key() })
            // Otherwise fall back to the current program's program_id.
            .unwrap_or(quote! { __program_id });
        // Convenience: how we store the bump so the caller can access it later.
        let bump_store = if f.is_optional {
            quote!(Some(__bump))
        } else {
            quote!(__bump)
        };

        // Build the PDA.
        let define_pda = match (&c.seeds, &c.bump) {
            // [list], no bump -> find_program_address + store __bump.
            (SeedsExpr::List(list), None) => {
                let maybe_seeds_plus_comma = (!list.is_empty()).then(|| quote! { #list, });
                quote! {
                    let (__pda_address, __bump) = Pubkey::find_program_address(
                        &[ #maybe_seeds_plus_comma ],
                        &#deriving_program_id,
                    );
                    __bumps.#name = #bump_store;
                }
            }

            // [list], explicit bump -> create_program_address with list + bump.
            (SeedsExpr::List(list), Some(b)) => {
                let maybe_seeds_plus_comma = (!list.is_empty()).then(|| quote! { #list, });
                quote! {
                    let __pda_address = Pubkey::create_program_address(
                        &[ #maybe_seeds_plus_comma &[#b][..] ],
                        &#deriving_program_id,
                    ).map_err(|_| anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds
                    ).with_account_name(#name_str))?;
                }
            }

            // expr, no bump -> find_program_address + store __bump.
            (SeedsExpr::Expr(expr), None) => quote! {
                let __seeds_slice: &[&[u8]] = #expr;
                let (__pda_address, __bump) =
                    Pubkey::find_program_address(__seeds_slice, &#deriving_program_id);
                __bumps.#name = #bump_store;
            },

            // expr, explicit bump -> concat slice + bump, then create_program_address.
            (SeedsExpr::Expr(expr), Some(b)) => quote! {
                let __bump_bytes = [#b];
                let __seeds_vec: ::std::vec::Vec<&[u8]> = [#expr, &[&__bump_bytes[..]]].concat();
                let __pda_address = Pubkey::create_program_address(
                    &__seeds_vec[..],
                    &#deriving_program_id,
                ).map_err(|_| anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::ConstraintSeeds
                ).with_account_name(#name_str))?;
            },
        };

        quote! {
            // Define the PDA.
            #define_pda

            // Check it.
            if #name.key() != __pda_address {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintSeeds).with_account_name(#name_str).with_pubkeys((#name.key(), __pda_address)));
            }
        }
    }
}

fn generate_constraint_associated_token(
    f: &Field,
    c: &ConstraintAssociatedToken,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let name = &f.ident;
    let name_str = name.to_string();
    let account_ref = generate_account_ref(f);
    let wallet_address = &c.wallet;
    let spl_token_mint_address = &c.mint;

    let mut optional_check_scope = OptionalCheckScope::new_with_field(accs, name);
    let wallet_address_optional_check = optional_check_scope.generate_check(wallet_address);
    let spl_token_mint_address_optional_check =
        optional_check_scope.generate_check(spl_token_mint_address);
    let optional_checks = quote! {
        #wallet_address_optional_check
        #spl_token_mint_address_optional_check
    };

    let token_program_check = match &c.token_program {
        Some(token_program) => {
            let token_program_optional_check = optional_check_scope.generate_check(token_program);
            quote! {
                #token_program_optional_check
                if #account_ref.owner != &#token_program.key() { return Err(anchor_lang::error::ErrorCode::ConstraintAssociatedTokenTokenProgram.into()); }
            }
        }
        None => quote! {},
    };
    let get_associated_token_address = match &c.token_program {
        Some(token_program) => quote! {
            ::anchor_spl::associated_token::get_associated_token_address_with_program_id(&wallet_address, &#spl_token_mint_address.key(), &#token_program.key())
        },
        None => quote! {
            ::anchor_spl::associated_token::get_associated_token_address(&wallet_address, &#spl_token_mint_address.key())
        },
    };

    quote! {
        {
            #optional_checks
            #token_program_check

            let my_owner = #name.owner;
            let wallet_address = #wallet_address.key();
            if my_owner != wallet_address {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintTokenOwner).with_account_name(#name_str).with_pubkeys((my_owner, wallet_address)));
            }
            let __associated_token_address = #get_associated_token_address;
            let my_key = #name.key();
            if my_key != __associated_token_address {
                return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintAssociated).with_account_name(#name_str).with_pubkeys((my_key, __associated_token_address)));
            }
        }
    }
}

fn generate_constraint_token_account(
    f: &Field,
    c: &ConstraintTokenAccountGroup,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let name = &f.ident;
    let name_str = name.to_string();
    let account_ref = generate_account_ref(f);
    let mut optional_check_scope = OptionalCheckScope::new_with_field(accs, name);

    let authority_check = match &c.authority {
        Some(authority) => {
            let authority_optional_check = optional_check_scope.generate_check(authority);
            quote! {
                #authority_optional_check
                if #name.owner != #authority.key() {
                    return Err(
                        anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintTokenOwner
                        )
                        .with_account_name(#name_str)
                        .with_pubkeys((#name.owner, #authority.key()))
                    );
                }
            }
        }
        None => quote! {},
    };

    let mint_check = match &c.mint {
        Some(mint) => {
            let mint_optional_check = optional_check_scope.generate_check(mint);
            quote! {
                #mint_optional_check
                if #name.mint != #mint.key() {
                    return Err(
                        anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintTokenMint
                        )
                        .with_account_name(#name_str)
                        .with_pubkeys((#name.mint, #mint.key()))
                    );
                }
            }
        }
        None => quote! {},
    };

    let token_program_check = match &c.token_program {
        Some(token_program) => {
            let token_program_optional_check = optional_check_scope.generate_check(token_program);
            quote! {
                #token_program_optional_check
                if #account_ref.owner != &#token_program.key() {
                    return Err(
                        anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintTokenTokenProgram
                        )
                        .with_account_name(#name_str)
                        .with_pubkeys((*#account_ref.owner, #token_program.key()))
                    );
                }
            }
        }
        None => quote! {},
    };

    quote! {
        {
            #authority_check
            #mint_check
            #token_program_check
        }
    }
}

fn generate_constraint_mint(
    f: &Field,
    c: &ConstraintTokenMintGroup,
    accs: &AccountsStruct,
) -> proc_macro2::TokenStream {
    let name = &f.ident;
    let account_ref = generate_account_ref(f);

    let decimal_check = match &c.decimals {
        Some(decimals) => quote! {
            if #name.decimals != #decimals {
                return Err(anchor_lang::error::ErrorCode::ConstraintMintDecimals.into());
            }
        },
        None => quote! {},
    };
    let mut optional_check_scope = OptionalCheckScope::new_with_field(accs, name);
    let mint_authority_check = match &c.mint_authority {
        Some(mint_authority) => {
            let mint_authority_optional_check = optional_check_scope.generate_check(mint_authority);
            quote! {
                #mint_authority_optional_check
                if #name.mint_authority != anchor_lang::solana_program::program_option::COption::Some(#mint_authority.key()) {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintMintAuthority.into());
                }
            }
        }
        None => quote! {},
    };
    let freeze_authority_check = match &c.freeze_authority {
        Some(freeze_authority) => {
            let freeze_authority_optional_check =
                optional_check_scope.generate_check(freeze_authority);
            quote! {
                #freeze_authority_optional_check
                if #name.freeze_authority != anchor_lang::solana_program::program_option::COption::Some(#freeze_authority.key()) {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintFreezeAuthority.into());
                }
            }
        }
        None => quote! {},
    };
    let token_program_check = match &c.token_program {
        Some(token_program) => {
            let token_program_optional_check = optional_check_scope.generate_check(token_program);
            quote! {
                #token_program_optional_check
                if #account_ref.owner != &#token_program.key() { return Err(anchor_lang::error::ErrorCode::ConstraintMintTokenProgram.into()); }
            }
        }
        None => quote! {},
    };

    let group_pointer_authority_check = match &c.group_pointer_authority {
        Some(group_pointer_authority) => {
            let group_pointer_authority_optional_check =
                optional_check_scope.generate_check(group_pointer_authority);
            quote! {
                let group_pointer = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::group_pointer::GroupPointer>(#account_ref);
                if group_pointer.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupPointerExtension.into());
                }
                #group_pointer_authority_optional_check
                if group_pointer.unwrap().authority != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#group_pointer_authority.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupPointerExtensionAuthority.into());
                }
            }
        }
        None => quote! {},
    };

    let group_pointer_group_address_check = match &c.group_pointer_group_address {
        Some(group_pointer_group_address) => {
            let group_pointer_group_address_optional_check =
                optional_check_scope.generate_check(group_pointer_group_address);
            quote! {
                let group_pointer = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::group_pointer::GroupPointer>(#account_ref);
                if group_pointer.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupPointerExtension.into());
                }
                #group_pointer_group_address_optional_check
                if group_pointer.unwrap().group_address != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#group_pointer_group_address.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupPointerExtensionGroupAddress.into());
                }
            }
        }
        None => quote! {},
    };

    let group_member_pointer_authority_check = match &c.group_member_pointer_authority {
        Some(group_member_pointer_authority) => {
            let group_member_pointer_authority_optional_check =
                optional_check_scope.generate_check(group_member_pointer_authority);
            quote! {
                let group_member_pointer = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::group_member_pointer::GroupMemberPointer>(#account_ref);
                if group_member_pointer.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupMemberPointerExtension.into());
                }
                #group_member_pointer_authority_optional_check
                if group_member_pointer.unwrap().authority != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#group_member_pointer_authority.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupMemberPointerExtensionAuthority.into());
                }
            }
        }
        None => quote! {},
    };

    let group_member_pointer_member_address_check = match &c.group_member_pointer_member_address {
        Some(group_member_pointer_member_address) => {
            let group_member_pointer_member_address_optional_check =
                optional_check_scope.generate_check(group_member_pointer_member_address);
            quote! {
                let group_member_pointer = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::group_member_pointer::GroupMemberPointer>(#account_ref);
                if group_member_pointer.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupMemberPointerExtension.into());
                }
                #group_member_pointer_member_address_optional_check
                if group_member_pointer.unwrap().member_address != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#group_member_pointer_member_address.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintGroupMemberPointerExtensionMemberAddress.into());
                }
            }
        }
        None => quote! {},
    };

    let metadata_pointer_authority_check = match &c.metadata_pointer_authority {
        Some(metadata_pointer_authority) => {
            let metadata_pointer_authority_optional_check =
                optional_check_scope.generate_check(metadata_pointer_authority);
            quote! {
                let metadata_pointer = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::metadata_pointer::MetadataPointer>(#account_ref);
                if metadata_pointer.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintMetadataPointerExtension.into());
                }
                #metadata_pointer_authority_optional_check
                if metadata_pointer.unwrap().authority != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#metadata_pointer_authority.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintMetadataPointerExtensionAuthority.into());
                }
            }
        }
        None => quote! {},
    };

    let metadata_pointer_metadata_address_check = match &c.metadata_pointer_metadata_address {
        Some(metadata_pointer_metadata_address) => {
            let metadata_pointer_metadata_address_optional_check =
                optional_check_scope.generate_check(metadata_pointer_metadata_address);
            quote! {
                let metadata_pointer = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::metadata_pointer::MetadataPointer>(#account_ref);
                if metadata_pointer.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintMetadataPointerExtension.into());
                }
                #metadata_pointer_metadata_address_optional_check
                if metadata_pointer.unwrap().metadata_address != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#metadata_pointer_metadata_address.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintMetadataPointerExtensionMetadataAddress.into());
                }
            }
        }
        None => quote! {},
    };

    let close_authority_check = match &c.close_authority {
        Some(close_authority) => {
            let close_authority_optional_check =
                optional_check_scope.generate_check(close_authority);
            quote! {
                let close_authority = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::mint_close_authority::MintCloseAuthority>(#account_ref);
                if close_authority.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintCloseAuthorityExtension.into());
                }
                #close_authority_optional_check
                if close_authority.unwrap().close_authority != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#close_authority.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintCloseAuthorityExtensionAuthority.into());
                }
            }
        }
        None => quote! {},
    };

    let permanent_delegate_check = match &c.permanent_delegate {
        Some(permanent_delegate) => {
            let permanent_delegate_optional_check =
                optional_check_scope.generate_check(permanent_delegate);
            quote! {
                let permanent_delegate = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::permanent_delegate::PermanentDelegate>(#account_ref);
                if permanent_delegate.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintPermanentDelegateExtension.into());
                }
                #permanent_delegate_optional_check
                if permanent_delegate.unwrap().delegate != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#permanent_delegate.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintPermanentDelegateExtensionDelegate.into());
                }
            }
        }
        None => quote! {},
    };

    let transfer_hook_authority_check = match &c.transfer_hook_authority {
        Some(transfer_hook_authority) => {
            let transfer_hook_authority_optional_check =
                optional_check_scope.generate_check(transfer_hook_authority);
            quote! {
                let transfer_hook = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::transfer_hook::TransferHook>(#account_ref);
                if transfer_hook.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintTransferHookExtension.into());
                }
                #transfer_hook_authority_optional_check
                if transfer_hook.unwrap().authority != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#transfer_hook_authority.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintTransferHookExtensionAuthority.into());
                }
            }
        }
        None => quote! {},
    };

    let transfer_hook_program_id_check = match &c.transfer_hook_program_id {
        Some(transfer_hook_program_id) => {
            let transfer_hook_program_id_optional_check =
                optional_check_scope.generate_check(transfer_hook_program_id);
            quote! {
                let transfer_hook = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::transfer_hook::TransferHook>(#account_ref);
                if transfer_hook.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintTransferHookExtension.into());
                }
                #transfer_hook_program_id_optional_check
                if transfer_hook.unwrap().program_id != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#transfer_hook_program_id.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintTransferHookExtensionProgramId.into());
                }
            }
        }
        None => quote! {},
    };

    let pausable_authority_check = match &c.pausable_authority {
        Some(pausable_authority) => {
            let pausable_authority_optional_check =
                optional_check_scope.generate_check(pausable_authority);
            quote! {
                let pausable = ::anchor_spl::token_interface::get_mint_extension_data::<::anchor_spl::token_interface::spl_token_2022::extension::pausable::PausableConfig>(#account_ref);
                if pausable.is_err() {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintPausableExtension.into());
                }
                #pausable_authority_optional_check
                if pausable.unwrap().authority != ::anchor_spl::token_2022_extensions::spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(#pausable_authority.key()))? {
                    return Err(anchor_lang::error::ErrorCode::ConstraintMintPausableAuthority.into());
                }
            }
        }
        None => quote! {},
    };

    quote! {
        {
            #decimal_check
            #mint_authority_check
            #freeze_authority_check
            #token_program_check
            #group_pointer_authority_check
            #group_pointer_group_address_check
            #group_member_pointer_authority_check
            #group_member_pointer_member_address_check
            #metadata_pointer_authority_check
            #metadata_pointer_metadata_address_check
            #close_authority_check
            #permanent_delegate_check
            #transfer_hook_authority_check
            #transfer_hook_program_id_check
            #pausable_authority_check
        }
    }
}

#[derive(Clone, Debug)]
pub struct OptionalCheckScope<'a> {
    seen: HashSet<String>,
    accounts: &'a AccountsStruct,
}

impl<'a> OptionalCheckScope<'a> {
    pub fn new(accounts: &'a AccountsStruct) -> Self {
        Self {
            seen: HashSet::new(),
            accounts,
        }
    }
    pub fn new_with_field(accounts: &'a AccountsStruct, field: impl ToString) -> Self {
        let mut check_scope = Self::new(accounts);
        check_scope.seen.insert(field.to_string());
        check_scope
    }
    pub fn generate_check(&mut self, field: impl ToTokens) -> TokenStream {
        let field_name = parser::tts_to_string(&field);
        if self.seen.contains(&field_name) {
            quote! {}
        } else {
            self.seen.insert(field_name.clone());
            if self.accounts.is_field_optional(&field) {
                quote! {
                    let #field = if let Some(ref account) = #field {
                        account
                    } else {
                        return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintAccountIsNone).with_account_name(#field_name));
                    };
                }
            } else {
                quote! {}
            }
        }
    }
}

fn generate_get_token_account_space(mint: &Expr) -> proc_macro2::TokenStream {
    quote! {
        {
            let mint_info = #mint.to_account_info();
            if *mint_info.owner == ::anchor_spl::token_2022::Token2022::id() {
                use ::anchor_spl::token_2022::spl_token_2022::extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions};
                use ::anchor_spl::token_2022::spl_token_2022::state::{Account, Mint};
                let mint_data = mint_info.try_borrow_data()?;
                let mint_state = StateWithExtensions::<Mint>::unpack(&mint_data)?;
                let mint_extensions = mint_state.get_extension_types()?;
                let required_extensions = ExtensionType::get_required_init_account_extensions(&mint_extensions);
                ExtensionType::try_calculate_account_len::<Account>(&required_extensions)?
            } else {
                ::anchor_spl::token::TokenAccount::LEN
            }
        }
    }
}

// Generated code to create an account or fund, allocate, and
// assign using the system program with the
// given `space` amount of data, owned by `owner`.
//
// `seeds_with_nonce` should be given for creating PDAs. Otherwise it's an
// empty stream.
//
// This should only be run within scopes where `system_program` is not Optional
fn generate_create_account_or_fund_allocate_assign(
    field: &Ident,
    space: proc_macro2::TokenStream,
    owner: proc_macro2::TokenStream,
    payer: proc_macro2::TokenStream,
    seeds_with_nonce: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    // Field, payer, and system program are already validated to not be an Option at this point
    quote! {
        // If the account being initialized already has lamports, then
        // fund the required lamports for rent exemption, allocate and assign
        let __current_lamports = #field.lamports();
        if __current_lamports == 0 {
            // Create the account with right amount of lamports and space, and the correct owner.
            let space = #space;
            let lamports = __anchor_rent.minimum_balance(space);
            let cpi_accounts = anchor_lang::system_program::CreateAccount {
                from: #payer.to_account_info(),
                to: #field.to_account_info()
            };
            let cpi_context = anchor_lang::context::CpiContext::new(system_program.key(), cpi_accounts);
            anchor_lang::system_program::create_account(cpi_context.with_signer(&[#seeds_with_nonce]), lamports, space as u64, #owner)?;
        } else {
            require_keys_neq!(#payer.key(), #field.key(), anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount);
            // Fund the account for rent exemption.
            let required_lamports = __anchor_rent
                .minimum_balance(#space)
                .max(1)
                .saturating_sub(__current_lamports);
            if required_lamports > 0 {
                let cpi_accounts = anchor_lang::system_program::Transfer {
                    from: #payer.to_account_info(),
                    to: #field.to_account_info(),
                };
                let cpi_context = anchor_lang::context::CpiContext::new(system_program.key(), cpi_accounts);
                anchor_lang::system_program::transfer(cpi_context, required_lamports)?;
            }

            let cpi_accounts = anchor_lang::system_program::Allocate {
                account_to_allocate: #field.to_account_info()
            };
            let cpi_context = anchor_lang::context::CpiContext::new(system_program.key(), cpi_accounts);
            anchor_lang::system_program::allocate(cpi_context.with_signer(&[#seeds_with_nonce]), #space as u64)?;

            let cpi_accounts = anchor_lang::system_program::Assign {
                account_to_assign: #field.to_account_info()
            };
            let cpi_context = anchor_lang::context::CpiContext::new(system_program.key(), cpi_accounts);
            anchor_lang::system_program::assign(cpi_context.with_signer(&[#seeds_with_nonce]), #owner)?;
        }
    }
}

pub fn generate_constraint_executable(
    f: &Field,
    _c: &ConstraintExecutable,
) -> proc_macro2::TokenStream {
    let name_str = f.ident.to_string();
    let account_ref = generate_account_ref(f);

    // because we are only acting on the field, we know it isn't optional at this point
    // as it was unwrapped in `generate_constraint`
    quote! {
        if !#account_ref.executable {
            return Err(anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::ConstraintExecutable).with_account_name(#name_str));
        }
    }
}

fn generate_custom_error(
    account_name: &Ident,
    custom_error: &Option<Expr>,
    error: proc_macro2::TokenStream,
    compared_values: &Option<&(proc_macro2::TokenStream, proc_macro2::TokenStream)>,
) -> proc_macro2::TokenStream {
    let account_name = account_name.to_string();
    let mut error = match custom_error {
        Some(error) => {
            quote! { anchor_lang::error::Error::from(#error).with_account_name(#account_name) }
        }
        None => {
            quote! { anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::#error).with_account_name(#account_name) }
        }
    };

    let compared_values = match compared_values {
        Some((left, right)) => quote! { .with_pubkeys((#left, #right)) },
        None => quote! {},
    };

    error.extend(compared_values);

    quote! {
        Err(#error)
    }
}

fn generate_account_ref(field: &Field) -> proc_macro2::TokenStream {
    let name = &field.ident;

    match &field.ty {
        Ty::AccountInfo => quote!(&#name),
        Ty::Account(acc) if acc.boxed => quote!(AsRef::<AccountInfo>::as_ref(#name.as_ref())),
        Ty::InterfaceAccount(acc) if acc.boxed => {
            quote!(AsRef::<AccountInfo>::as_ref(#name.as_ref()))
        }
        _ => quote!(AsRef::<AccountInfo>::as_ref(&#name)),
    }
}
