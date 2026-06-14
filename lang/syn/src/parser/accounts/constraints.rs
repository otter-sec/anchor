use {
    crate::*,
    syn::{
        parse::{Error as ParseError, Result as ParseResult},
        Expr, Ident, Token,
    },
};

pub fn parse(f: &syn::Field, f_ty: Option<&Ty>) -> ParseResult<ConstraintGroup> {
    let mut constraints = ConstraintGroupBuilder::new(f_ty);
    for attr in f.attrs.iter().filter(is_account) {
        for c in attr.parse_args_with(Punctuated::<ConstraintToken, Comma>::parse_terminated)? {
            constraints.add(c)?;
        }
    }
    let account_constraints = constraints.build()?;

    Ok(account_constraints)
}

pub fn is_account(attr: &&syn::Attribute) -> bool {
    attr.path()
        .get_ident()
        .is_some_and(|ident| ident == "account")
}

// Parses a single constraint from a parse stream for `#[account(<STREAM>)]`.
pub fn parse_token(stream: ParseStream) -> ParseResult<ConstraintToken> {
    let ident = stream.call(Ident::parse_any)?;
    let kw = ident.to_string();

    let c = match kw.as_str() {
        "init" => ConstraintToken::Init(Context::new(
            ident.span(),
            ConstraintInit { if_needed: false },
        )),
        "init_if_needed" => ConstraintToken::Init(Context::new(
            ident.span(),
            ConstraintInit { if_needed: true },
        )),
        "zero" => ConstraintToken::Zeroed(Context::new(ident.span(), ConstraintZeroed {})),
        "mut" => ConstraintToken::Mut(Context::new(
            ident.span(),
            ConstraintMut {
                error: parse_optional_custom_error(&stream)?,
            },
        )),
        "signer" => ConstraintToken::Signer(Context::new(
            ident.span(),
            ConstraintSigner {
                error: parse_optional_custom_error(&stream)?,
            },
        )),
        "executable" => {
            ConstraintToken::Executable(Context::new(ident.span(), ConstraintExecutable {}))
        }
        "dup" => ConstraintToken::Dup(Context::new(ident.span(), ConstraintDup {})),
        "mint" => {
            stream.parse::<Token![:]>()?;
            stream.parse::<Token![:]>()?;
            let kw = stream.call(Ident::parse_any)?.to_string();
            stream.parse::<Token![=]>()?;

            let span = ident
                .span()
                .join(stream.span())
                .unwrap_or_else(|| ident.span());

            match kw.as_str() {
                "authority" => ConstraintToken::MintAuthority(Context::new(
                    span,
                    ConstraintMintAuthority {
                        mint_auth: stream.parse()?,
                    },
                )),
                "freeze_authority" => ConstraintToken::MintFreezeAuthority(Context::new(
                    span,
                    ConstraintMintFreezeAuthority {
                        mint_freeze_auth: stream.parse()?,
                    },
                )),
                "decimals" => ConstraintToken::MintDecimals(Context::new(
                    span,
                    ConstraintMintDecimals {
                        decimals: stream.parse()?,
                    },
                )),
                "token_program" => ConstraintToken::MintTokenProgram(Context::new(
                    span,
                    ConstraintTokenProgram {
                        token_program: stream.parse()?,
                    },
                )),
                _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
            }
        }
        "extensions" => {
            stream.parse::<Token![:]>()?;
            stream.parse::<Token![:]>()?;
            let kw = stream.call(Ident::parse_any)?.to_string();

            match kw.as_str() {
                "group_pointer" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "authority" => {
                            ConstraintToken::ExtensionGroupPointerAuthority(Context::new(
                                span,
                                ConstraintExtensionAuthority {
                                    authority: stream.parse()?,
                                },
                            ))
                        }
                        "group_address" => {
                            ConstraintToken::ExtensionGroupPointerGroupAddress(Context::new(
                                span,
                                ConstraintExtensionGroupPointerGroupAddress {
                                    group_address: stream.parse()?,
                                },
                            ))
                        }
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                "group_member_pointer" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "authority" => {
                            ConstraintToken::ExtensionGroupMemberPointerAuthority(Context::new(
                                span,
                                ConstraintExtensionAuthority {
                                    authority: stream.parse()?,
                                },
                            ))
                        }
                        "member_address" => {
                            ConstraintToken::ExtensionGroupMemberPointerMemberAddress(Context::new(
                                span,
                                ConstraintExtensionGroupMemberPointerMemberAddress {
                                    member_address: stream.parse()?,
                                },
                            ))
                        }
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                "metadata_pointer" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "authority" => {
                            ConstraintToken::ExtensionMetadataPointerAuthority(Context::new(
                                span,
                                ConstraintExtensionAuthority {
                                    authority: stream.parse()?,
                                },
                            ))
                        }
                        "metadata_address" => {
                            ConstraintToken::ExtensionMetadataPointerMetadataAddress(Context::new(
                                span,
                                ConstraintExtensionMetadataPointerMetadataAddress {
                                    metadata_address: stream.parse()?,
                                },
                            ))
                        }
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                "close_authority" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "authority" => ConstraintToken::ExtensionCloseAuthority(Context::new(
                            span,
                            ConstraintExtensionAuthority {
                                authority: stream.parse()?,
                            },
                        )),
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                "permanent_delegate" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "delegate" => ConstraintToken::ExtensionPermanentDelegate(Context::new(
                            span,
                            ConstraintExtensionPermanentDelegate {
                                permanent_delegate: stream.parse()?,
                            },
                        )),
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                "transfer_hook" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "authority" => ConstraintToken::ExtensionTokenHookAuthority(Context::new(
                            span,
                            ConstraintExtensionAuthority {
                                authority: stream.parse()?,
                            },
                        )),
                        "program_id" => ConstraintToken::ExtensionTokenHookProgramId(Context::new(
                            span,
                            ConstraintExtensionTokenHookProgramId {
                                program_id: stream.parse()?,
                            },
                        )),
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                "pausable" => {
                    stream.parse::<Token![:]>()?;
                    stream.parse::<Token![:]>()?;
                    let kw = stream.call(Ident::parse_any)?.to_string();
                    stream.parse::<Token![=]>()?;

                    let span = ident
                        .span()
                        .join(stream.span())
                        .unwrap_or_else(|| ident.span());

                    match kw.as_str() {
                        "authority" => ConstraintToken::ExtensionPausableAuthority(Context::new(
                            span,
                            ConstraintExtensionAuthority {
                                authority: stream.parse()?,
                            },
                        )),
                        _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                    }
                }
                _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
            }
        }
        "token" => {
            stream.parse::<Token![:]>()?;
            stream.parse::<Token![:]>()?;
            let kw = stream.call(Ident::parse_any)?.to_string();
            stream.parse::<Token![=]>()?;

            let span = ident
                .span()
                .join(stream.span())
                .unwrap_or_else(|| ident.span());

            match kw.as_str() {
                "mint" => ConstraintToken::TokenMint(Context::new(
                    span,
                    ConstraintTokenMint {
                        mint: stream.parse()?,
                    },
                )),
                "authority" => ConstraintToken::TokenAuthority(Context::new(
                    span,
                    ConstraintTokenAuthority {
                        auth: stream.parse()?,
                    },
                )),
                "token_program" => ConstraintToken::TokenTokenProgram(Context::new(
                    span,
                    ConstraintTokenProgram {
                        token_program: stream.parse()?,
                    },
                )),
                _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
            }
        }
        "associated_token" => {
            stream.parse::<Token![:]>()?;
            stream.parse::<Token![:]>()?;
            let kw = stream.call(Ident::parse_any)?.to_string();
            stream.parse::<Token![=]>()?;

            let span = ident
                .span()
                .join(stream.span())
                .unwrap_or_else(|| ident.span());

            match kw.as_str() {
                "mint" => ConstraintToken::AssociatedTokenMint(Context::new(
                    span,
                    ConstraintTokenMint {
                        mint: stream.parse()?,
                    },
                )),
                "authority" => ConstraintToken::AssociatedTokenAuthority(Context::new(
                    span,
                    ConstraintTokenAuthority {
                        auth: stream.parse()?,
                    },
                )),
                "token_program" => ConstraintToken::AssociatedTokenTokenProgram(Context::new(
                    span,
                    ConstraintTokenProgram {
                        token_program: stream.parse()?,
                    },
                )),
                _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
            }
        }
        "bump" => {
            let bump = {
                if stream.peek(Token![=]) {
                    stream.parse::<Token![=]>()?;
                    Some(stream.parse()?)
                } else {
                    None
                }
            };
            ConstraintToken::Bump(Context::new(ident.span(), ConstraintTokenBump { bump }))
        }
        "seeds" => {
            if stream.peek(Token![:]) {
                stream.parse::<Token![:]>()?;
                stream.parse::<Token![:]>()?;
                let kw = stream.call(Ident::parse_any)?.to_string();
                stream.parse::<Token![=]>()?;

                let span = ident
                    .span()
                    .join(stream.span())
                    .unwrap_or_else(|| ident.span());

                match kw.as_str() {
                    "program" => ConstraintToken::ProgramSeed(Context::new(
                        span,
                        ConstraintProgramSeed {
                            program_seed: stream.parse()?,
                        },
                    )),
                    _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
                }
            } else {
                stream.parse::<Token![=]>()?;
                let span = ident
                    .span()
                    .join(stream.span())
                    .unwrap_or_else(|| ident.span());

                let seeds_expr: SeedsExpr = stream.parse()?;
                ConstraintToken::Seeds(Context::new(span, ConstraintSeeds { seeds: seeds_expr }))
            }
        }
        "realloc" => {
            if stream.peek(Token![=]) {
                stream.parse::<Token![=]>()?;
                let span = ident
                    .span()
                    .join(stream.span())
                    .unwrap_or_else(|| ident.span());
                ConstraintToken::Realloc(Context::new(
                    span,
                    ConstraintRealloc {
                        space: stream.parse()?,
                    },
                ))
            } else {
                stream.parse::<Token![:]>()?;
                stream.parse::<Token![:]>()?;
                let kw = stream.call(Ident::parse_any)?.to_string();
                stream.parse::<Token![=]>()?;

                let span = ident
                    .span()
                    .join(stream.span())
                    .unwrap_or_else(|| ident.span());

                match kw.as_str() {
                    "payer" => ConstraintToken::ReallocPayer(Context::new(
                        span,
                        ConstraintReallocPayer {
                            target: stream.parse()?,
                        },
                    )),
                    "zero" => ConstraintToken::ReallocZero(Context::new(
                        span,
                        ConstraintReallocZero {
                            zero: stream.parse()?,
                        },
                    )),
                    _ => {
                        return Err(ParseError::new(
                            ident.span(),
                            "Invalid attribute. realloc::payer and realloc::zero are the only \
                             valid attributes",
                        ))
                    }
                }
            }
        }
        _ => {
            stream.parse::<Token![=]>()?;
            let span = ident
                .span()
                .join(stream.span())
                .unwrap_or_else(|| ident.span());
            match kw.as_str() {
                "has_one" => ConstraintToken::HasOne(Context::new(
                    span,
                    ConstraintHasOne {
                        join_target: stream.parse()?,
                        error: parse_optional_custom_error(&stream)?,
                    },
                )),
                "owner" => ConstraintToken::Owner(Context::new(
                    span,
                    ConstraintOwner {
                        owner_address: stream.parse()?,
                        error: parse_optional_custom_error(&stream)?,
                    },
                )),
                "rent_exempt" => ConstraintToken::RentExempt(Context::new(
                    span,
                    match stream.parse::<Ident>()?.to_string().as_str() {
                        "skip" => ConstraintRentExempt::Skip,
                        "enforce" => ConstraintRentExempt::Enforce,
                        _ => {
                            return Err(ParseError::new(
                                span,
                                "rent_exempt must be either skip or enforce",
                            ))
                        }
                    },
                )),
                "payer" => ConstraintToken::Payer(Context::new(
                    span,
                    ConstraintPayer {
                        target: stream.parse()?,
                    },
                )),
                "space" => ConstraintToken::Space(Context::new(
                    span,
                    ConstraintSpace {
                        space: stream.parse()?,
                    },
                )),
                "constraint" => ConstraintToken::Raw(Context::new(
                    span,
                    ConstraintRaw {
                        raw: {
                            let expr = stream.parse()?;

                            if let syn::Expr::Lit(syn::ExprLit { lit, .. }) = &expr {
                                if !matches!(lit, syn::Lit::Bool(_)) {
                                    return Err(ParseError::new(
                                        expr.span(),
                                        "constraint must be a boolean expression, not a \
                                         literal.\nHelp: Raw constraints expect expressions that \
                                         evaluate to boolean values.\nIf you need to compare a \
                                         field to a value, use: constraint = my_field == <value>",
                                    ));
                                }
                            }

                            expr
                        },
                        error: parse_optional_custom_error(&stream)?,
                    },
                )),
                "close" => ConstraintToken::Close(Context::new(
                    span,
                    ConstraintClose {
                        sol_dest: stream.parse()?,
                    },
                )),
                "address" => ConstraintToken::Address(Context::new(
                    span,
                    ConstraintAddress {
                        address: stream.parse()?,
                        error: parse_optional_custom_error(&stream)?,
                    },
                )),
                _ => return Err(ParseError::new(ident.span(), "Invalid attribute")),
            }
        }
    };

    Ok(c)
}

fn parse_optional_custom_error(stream: &ParseStream) -> ParseResult<Option<Expr>> {
    if stream.peek(Token![@]) {
        stream.parse::<Token![@]>()?;
        stream.parse().map(Some)
    } else {
        Ok(None)
    }
}

#[derive(Default)]
pub struct ConstraintGroupBuilder<'ty> {
    pub f_ty: Option<&'ty Ty>,
    pub init: Option<Context<ConstraintInit>>,
    pub zeroed: Option<Context<ConstraintZeroed>>,
    pub mutable: Option<Context<ConstraintMut>>,
    pub signer: Option<Context<ConstraintSigner>>,
    pub has_one: Vec<Context<ConstraintHasOne>>,
    pub raw: Vec<Context<ConstraintRaw>>,
    pub owner: Option<Context<ConstraintOwner>>,
    pub rent_exempt: Option<Context<ConstraintRentExempt>>,
    pub seeds: Option<Context<ConstraintSeeds>>,
    pub executable: Option<Context<ConstraintExecutable>>,
    pub payer: Option<Context<ConstraintPayer>>,
    pub space: Option<Context<ConstraintSpace>>,
    pub close: Option<Context<ConstraintClose>>,
    pub address: Option<Context<ConstraintAddress>>,
    pub token_mint: Option<Context<ConstraintTokenMint>>,
    pub token_authority: Option<Context<ConstraintTokenAuthority>>,
    pub token_token_program: Option<Context<ConstraintTokenProgram>>,
    pub associated_token_mint: Option<Context<ConstraintTokenMint>>,
    pub associated_token_authority: Option<Context<ConstraintTokenAuthority>>,
    pub associated_token_token_program: Option<Context<ConstraintTokenProgram>>,
    pub mint_authority: Option<Context<ConstraintMintAuthority>>,
    pub mint_freeze_authority: Option<Context<ConstraintMintFreezeAuthority>>,
    pub mint_decimals: Option<Context<ConstraintMintDecimals>>,
    pub mint_token_program: Option<Context<ConstraintTokenProgram>>,
    pub extension_group_pointer_authority: Option<Context<ConstraintExtensionAuthority>>,
    pub extension_group_pointer_group_address:
        Option<Context<ConstraintExtensionGroupPointerGroupAddress>>,
    pub extension_group_member_pointer_authority: Option<Context<ConstraintExtensionAuthority>>,
    pub extension_group_member_pointer_member_address:
        Option<Context<ConstraintExtensionGroupMemberPointerMemberAddress>>,
    pub extension_metadata_pointer_authority: Option<Context<ConstraintExtensionAuthority>>,
    pub extension_metadata_pointer_metadata_address:
        Option<Context<ConstraintExtensionMetadataPointerMetadataAddress>>,
    pub extension_close_authority: Option<Context<ConstraintExtensionAuthority>>,
    pub extension_transfer_hook_authority: Option<Context<ConstraintExtensionAuthority>>,
    pub extension_transfer_hook_program_id: Option<Context<ConstraintExtensionTokenHookProgramId>>,
    pub extension_permanent_delegate: Option<Context<ConstraintExtensionPermanentDelegate>>,
    pub extension_pausable_authority: Option<Context<ConstraintExtensionAuthority>>,
    pub bump: Option<Context<ConstraintTokenBump>>,
    pub program_seed: Option<Context<ConstraintProgramSeed>>,
    pub realloc: Option<Context<ConstraintRealloc>>,
    pub realloc_payer: Option<Context<ConstraintReallocPayer>>,
    pub realloc_zero: Option<Context<ConstraintReallocZero>>,
    pub dup: Option<Context<ConstraintDup>>,
}

impl<'ty> ConstraintGroupBuilder<'ty> {
    pub fn new(f_ty: Option<&'ty Ty>) -> Self {
        Self {
            f_ty,
            init: None,
            zeroed: None,
            mutable: None,
            signer: None,
            has_one: Vec::new(),
            raw: Vec::new(),
            owner: None,
            rent_exempt: None,
            seeds: None,
            executable: None,
            payer: None,
            space: None,
            close: None,
            address: None,
            token_mint: None,
            token_authority: None,
            token_token_program: None,
            associated_token_mint: None,
            associated_token_authority: None,
            associated_token_token_program: None,
            mint_authority: None,
            mint_freeze_authority: None,
            mint_decimals: None,
            mint_token_program: None,
            extension_group_pointer_authority: None,
            extension_group_pointer_group_address: None,
            extension_group_member_pointer_authority: None,
            extension_group_member_pointer_member_address: None,
            extension_metadata_pointer_authority: None,
            extension_metadata_pointer_metadata_address: None,
            extension_close_authority: None,
            extension_transfer_hook_authority: None,
            extension_transfer_hook_program_id: None,
            extension_permanent_delegate: None,
            extension_pausable_authority: None,
            bump: None,
            program_seed: None,
            realloc: None,
            realloc_payer: None,
            realloc_zero: None,
            dup: None,
        }
    }

    pub fn build(mut self) -> ParseResult<ConstraintGroup> {
        // Init.
        if let Some(i) = &self.init {
            if cfg!(not(feature = "init-if-needed")) && i.if_needed {
                return Err(ParseError::new(
                    i.span(),
                    "init_if_needed requires that anchor-lang be imported with the init-if-needed \
                     cargo feature enabled. Carefully read the init_if_needed docs before using \
                     this feature to make sure you know how to protect yourself against \
                     re-initialization attacks.",
                ));
            }

            match self.mutable {
                Some(m) => {
                    return Err(ParseError::new(
                        m.span(),
                        "mut cannot be provided with init",
                    ))
                }
                None => self
                    .mutable
                    .replace(Context::new(i.span(), ConstraintMut { error: None })),
            };
            // Rent exempt if not explicitly skipped.
            if self.rent_exempt.is_none() {
                self.rent_exempt
                    .replace(Context::new(i.span(), ConstraintRentExempt::Enforce));
            }
            if self.payer.is_none() {
                return Err(ParseError::new(
                    i.span(),
                    "payer must be provided when initializing an account",
                ));
            }
            // When initializing a non-PDA account, the account being
            // initialized must sign to invoke the system program's create
            // account instruction.
            if self.signer.is_none() && self.seeds.is_none() && self.associated_token_mint.is_none()
            {
                self.signer
                    .replace(Context::new(i.span(), ConstraintSigner { error: None }));
            }

            // Assert a bump target is not given on init.
            if let Some(b) = &self.bump {
                if b.bump.is_some() {
                    return Err(ParseError::new(
                        b.span(),
                        "bump targets should not be provided with init. Please use bump without a \
                         target.",
                    ));
                }
            }

            // TokenAccount.
            if let Some(token_mint) = &self.token_mint {
                if self.token_authority.is_none() {
                    return Err(ParseError::new(
                        token_mint.span(),
                        "when initializing, token authority must be provided if token mint is",
                    ));
                }
            }
            if let Some(token_authority) = &self.token_authority {
                if self.token_mint.is_none() {
                    return Err(ParseError::new(
                        token_authority.span(),
                        "when initializing, token mint must be provided if token authority is",
                    ));
                }
            }

            // Mint.
            if let Some(mint_decimals) = &self.mint_decimals {
                if self.mint_authority.is_none() {
                    return Err(ParseError::new(
                        mint_decimals.span(),
                        "when initializing, mint authority must be provided if mint decimals is",
                    ));
                }
            }
            if let Some(mint_authority) = &self.mint_authority {
                if self.mint_decimals.is_none() {
                    return Err(ParseError::new(
                        mint_authority.span(),
                        "when initializing, mint decimals must be provided if mint authority is",
                    ));
                }
            }
        }

        // Realloc.
        if let Some(r) = &self.realloc {
            if self.realloc_payer.is_none() {
                return Err(ParseError::new(
                    r.span(),
                    "realloc::payer must be provided when using realloc",
                ));
            }
            if self.realloc_zero.is_none() {
                return Err(ParseError::new(
                    r.span(),
                    "realloc::zero must be provided when using realloc",
                ));
            }
        }

        // Zero.
        if let Some(z) = &self.zeroed {
            match self.mutable {
                Some(m) => {
                    return Err(ParseError::new(
                        m.span(),
                        "mut cannot be provided with zeroed",
                    ))
                }
                None => self
                    .mutable
                    .replace(Context::new(z.span(), ConstraintMut { error: None })),
            };
            // Rent exempt if not explicitly skipped.
            if self.rent_exempt.is_none() {
                self.rent_exempt
                    .replace(Context::new(z.span(), ConstraintRentExempt::Enforce));
            }
        }

        // Seeds.
        if let Some(i) = &self.seeds {
            if self.init.is_some() && self.payer.is_none() {
                return Err(ParseError::new(
                    i.span(),
                    "payer must be provided when creating a program derived address",
                ));
            }
            if self.bump.is_none() {
                return Err(ParseError::new(
                    i.span(),
                    "bump must be provided with seeds",
                ));
            }
        }

        // Space.
        if let Some(i) = &self.init {
            let initializing_token_program_acc = self.token_mint.is_some()
                || self.mint_authority.is_some()
                || self.token_authority.is_some()
                || self.associated_token_authority.is_some();

            match (self.space.is_some(), initializing_token_program_acc) {
                (true, true) => {
                    #[allow(
                        clippy::unwrap_used,
                        reason = "inside (true, _) arm so space is guaranteed Some"
                    )]
                    return Err(ParseError::new(
                        self.space.as_ref().unwrap().span(),
                        "space is not required for initializing an spl account",
                    ));
                }
                (false, false) => {
                    return Err(ParseError::new(
                        i.span(),
                        "space must be provided with init",
                    ));
                }
                _ => (),
            }
        }

        let ConstraintGroupBuilder {
            f_ty: _,
            init,
            zeroed,
            mutable,
            signer,
            has_one,
            raw,
            owner,
            rent_exempt,
            seeds,
            executable,
            payer,
            space,
            close,
            address,
            token_mint,
            token_authority,
            token_token_program,
            associated_token_mint,
            associated_token_authority,
            associated_token_token_program,
            mint_authority,
            mint_freeze_authority,
            mint_decimals,
            mint_token_program,
            extension_group_pointer_authority,
            extension_group_pointer_group_address,
            extension_group_member_pointer_authority,
            extension_group_member_pointer_member_address,
            extension_metadata_pointer_authority,
            extension_metadata_pointer_metadata_address,
            extension_close_authority,
            extension_transfer_hook_authority,
            extension_transfer_hook_program_id,
            extension_permanent_delegate,
            extension_pausable_authority,
            bump,
            program_seed,
            realloc,
            realloc_payer,
            realloc_zero,
            dup,
        } = self;

        // Converts Option<Context<T>> -> Option<T>.
        macro_rules! into_inner {
            ($opt:ident) => {
                $opt.map(|c| c.into_inner())
            };
            ($opt:expr) => {
                $opt.map(|c| c.into_inner())
            };
        }
        // Converts Vec<Context<T>> - Vec<T>.
        macro_rules! into_inner_vec {
            ($opt:ident) => {
                $opt.into_iter().map(|c| c.into_inner()).collect()
            };
        }

        let is_init = init.is_some();
        #[allow(
            clippy::expect_used,
            reason = "bump presence validated earlier in constraint parsing"
        )]
        let seeds = seeds.map(|c| ConstraintSeedsGroup {
            is_init,
            seeds: c.seeds.clone(),
            bump: into_inner!(bump)
                .map(|b| b.bump)
                .expect("bump must be provided with seeds"),
            program_seed: into_inner!(program_seed).map(|id| id.program_seed),
        });
        let associated_token = match (
            associated_token_mint,
            associated_token_authority,
            &associated_token_token_program,
        ) {
            (Some(mint), Some(auth), _) => Some(ConstraintAssociatedToken {
                wallet: auth.into_inner().auth,
                mint: mint.into_inner().mint,
                token_program: associated_token_token_program
                    .as_ref()
                    .map(|a| a.clone().into_inner().token_program),
            }),
            (Some(mint), None, _) => {
                return Err(ParseError::new(
                    mint.span(),
                    "authority must be provided to specify an associated token program derived \
                     address",
                ))
            }
            (None, Some(auth), _) => {
                return Err(ParseError::new(
                    auth.span(),
                    "mint must be provided to specify an associated token program derived address",
                ))
            }
            (None, None, Some(token_program)) => {
                return Err(ParseError::new(
                    token_program.span(),
                    "mint and authority must be provided to specify an associated token program \
                     derived address",
                ))
            }
            _ => None,
        };
        if let Some(associated_token) = &associated_token {
            if seeds.is_some() {
                return Err(ParseError::new(
                    associated_token.mint.span(),
                    "'associated_token' constraints cannot be used with the 'seeds' constraint",
                ));
            }
        }

        let token_account = match (&token_mint, &token_authority, &token_token_program) {
            (None, None, None) => None,
            _ => Some(ConstraintTokenAccountGroup {
                mint: token_mint.as_ref().map(|a| a.clone().into_inner().mint),
                authority: token_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().auth),
                token_program: token_token_program
                    .as_ref()
                    .map(|a| a.clone().into_inner().token_program),
            }),
        };

        let mint = match (
            &mint_decimals,
            &mint_authority,
            &mint_freeze_authority,
            &mint_token_program,
            &extension_group_pointer_authority,
            &extension_group_pointer_group_address,
            &extension_group_member_pointer_authority,
            &extension_group_member_pointer_member_address,
            &extension_metadata_pointer_authority,
            &extension_metadata_pointer_metadata_address,
            &extension_close_authority,
            &extension_transfer_hook_authority,
            &extension_transfer_hook_program_id,
            &extension_permanent_delegate,
            &extension_pausable_authority,
        ) {
            (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ) => None,
            _ => Some(ConstraintTokenMintGroup {
                decimals: mint_decimals
                    .as_ref()
                    .map(|a| a.clone().into_inner().decimals),
                mint_authority: mint_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().mint_auth),
                freeze_authority: mint_freeze_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().mint_freeze_auth),
                token_program: mint_token_program
                    .as_ref()
                    .map(|a| a.clone().into_inner().token_program),
                // extensions
                group_pointer_authority: extension_group_pointer_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().authority),
                group_pointer_group_address: extension_group_pointer_group_address
                    .as_ref()
                    .map(|a| a.clone().into_inner().group_address),
                group_member_pointer_authority: extension_group_member_pointer_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().authority),
                group_member_pointer_member_address: extension_group_member_pointer_member_address
                    .as_ref()
                    .map(|a| a.clone().into_inner().member_address),
                metadata_pointer_authority: extension_metadata_pointer_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().authority),
                metadata_pointer_metadata_address: extension_metadata_pointer_metadata_address
                    .as_ref()
                    .map(|a| a.clone().into_inner().metadata_address),
                close_authority: extension_close_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().authority),
                permanent_delegate: extension_permanent_delegate
                    .as_ref()
                    .map(|a| a.clone().into_inner().permanent_delegate),
                transfer_hook_authority: extension_transfer_hook_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().authority),
                transfer_hook_program_id: extension_transfer_hook_program_id
                    .as_ref()
                    .map(|a| a.clone().into_inner().program_id),
                pausable_authority: extension_pausable_authority
                    .as_ref()
                    .map(|a| a.clone().into_inner().authority),
            }),
        };

        Ok(ConstraintGroup {
            init: init
                .as_ref()
                .map(|i| {
                    Ok(ConstraintInitGroup {
                        if_needed: i.if_needed,
                        seeds: seeds.clone(),
                        #[allow(
                            clippy::unwrap_used,
                            reason = "payer is guaranteed present when init constraint exists"
                        )]
                        payer: into_inner!(payer.clone()).unwrap().target,
                        space: space.clone().map(|s| s.space.clone()),
                        kind: if let Some(tm) = &token_mint {
                            InitKind::Token {
                                mint: tm.clone().into_inner().mint,
                                owner: match &token_authority {
                                    Some(a) => a.clone().into_inner().auth,
                                    None => {
                                        return Err(ParseError::new(
                                            tm.span(),
                                            "authority must be provided to initialize a token \
                                             program derived address",
                                        ))
                                    }
                                },
                                token_program: token_token_program
                                    .map(|tp| tp.into_inner().token_program),
                            }
                        } else if let Some(at) = &associated_token {
                            InitKind::AssociatedToken {
                                mint: at.mint.clone(),
                                owner: at.wallet.clone(),
                                token_program: associated_token_token_program
                                    .map(|tp| tp.into_inner().token_program),
                            }
                        } else if let Some(d) = &mint_decimals {
                            InitKind::Mint {
                                decimals: d.clone().into_inner().decimals,
                                owner: match &mint_authority {
                                    Some(a) => a.clone().into_inner().mint_auth,
                                    None => {
                                        return Err(ParseError::new(
                                            d.span(),
                                            "authority must be provided to initialize a mint \
                                             program derived address",
                                        ))
                                    }
                                },
                                freeze_authority: mint_freeze_authority
                                    .map(|fa| fa.into_inner().mint_freeze_auth),
                                token_program: mint_token_program
                                    .map(|tp| tp.into_inner().token_program),
                                // extensions
                                group_pointer_authority: extension_group_pointer_authority
                                    .map(|gpa| gpa.into_inner().authority),
                                group_pointer_group_address: extension_group_pointer_group_address
                                    .map(|gpga| gpga.into_inner().group_address),
                                group_member_pointer_authority:
                                    extension_group_member_pointer_authority
                                        .map(|gmpa| gmpa.into_inner().authority),
                                group_member_pointer_member_address:
                                    extension_group_member_pointer_member_address
                                        .map(|gmpma| gmpma.into_inner().member_address),
                                metadata_pointer_authority: extension_metadata_pointer_authority
                                    .map(|mpa| mpa.into_inner().authority),
                                metadata_pointer_metadata_address:
                                    extension_metadata_pointer_metadata_address
                                        .map(|mpma| mpma.into_inner().metadata_address),
                                close_authority: extension_close_authority
                                    .map(|ca| ca.into_inner().authority),
                                permanent_delegate: extension_permanent_delegate
                                    .map(|pd| pd.into_inner().permanent_delegate),
                                transfer_hook_authority: extension_transfer_hook_authority
                                    .map(|tha| tha.into_inner().authority),
                                transfer_hook_program_id: extension_transfer_hook_program_id
                                    .map(|thpid| thpid.into_inner().program_id),
                                pausable_authority: extension_pausable_authority
                                    .map(|ca| ca.into_inner().authority),
                            }
                        } else {
                            InitKind::Program {
                                owner: owner.as_ref().map(|o| o.owner_address.clone()),
                            }
                        },
                    })
                })
                .transpose()?,
            realloc: realloc.as_ref().map(|r| ConstraintReallocGroup {
                #[allow(
                    clippy::unwrap_used,
                    reason = "realloc payer guaranteed when realloc constraint present"
                )]
                payer: into_inner!(realloc_payer).unwrap().target,
                space: r.space.clone(),
                #[allow(
                    clippy::unwrap_used,
                    reason = "realloc zero guaranteed when realloc constraint present"
                )]
                zero: into_inner!(realloc_zero).unwrap().zero,
            }),
            zeroed: into_inner!(zeroed),
            mutable: into_inner!(mutable),
            signer: into_inner!(signer),
            has_one: into_inner_vec!(has_one),
            raw: into_inner_vec!(raw),
            owner: into_inner!(owner),
            rent_exempt: into_inner!(rent_exempt),
            executable: into_inner!(executable),
            close: into_inner!(close),
            address: into_inner!(address),
            associated_token: if !is_init { associated_token } else { None },
            seeds,
            token_account: if !is_init { token_account } else { None },
            mint: if !is_init { mint } else { None },
            dup: into_inner!(dup),
        })
    }

    pub fn add(&mut self, c: ConstraintToken) -> ParseResult<()> {
        match c {
            ConstraintToken::Init(c) => self.add_init(c),
            ConstraintToken::Zeroed(c) => self.add_zeroed(c),
            ConstraintToken::Mut(c) => self.add_mut(c),
            ConstraintToken::Signer(c) => self.add_signer(c),
            ConstraintToken::HasOne(c) => self.add_has_one(c),
            ConstraintToken::Raw(c) => self.add_raw(c),
            ConstraintToken::Owner(c) => self.add_owner(c),
            ConstraintToken::RentExempt(c) => self.add_rent_exempt(c),
            ConstraintToken::Seeds(c) => self.add_seeds(c),
            ConstraintToken::Executable(c) => self.add_executable(c),
            ConstraintToken::Payer(c) => self.add_payer(c),
            ConstraintToken::Space(c) => self.add_space(c),
            ConstraintToken::Close(c) => self.add_close(c),
            ConstraintToken::Address(c) => self.add_address(c),
            ConstraintToken::TokenAuthority(c) => self.add_token_authority(c),
            ConstraintToken::TokenMint(c) => self.add_token_mint(c),
            ConstraintToken::TokenTokenProgram(c) => self.add_token_token_program(c),
            ConstraintToken::AssociatedTokenAuthority(c) => self.add_associated_token_authority(c),
            ConstraintToken::AssociatedTokenMint(c) => self.add_associated_token_mint(c),
            ConstraintToken::AssociatedTokenTokenProgram(c) => {
                self.add_associated_token_token_program(c)
            }
            ConstraintToken::MintAuthority(c) => self.add_mint_authority(c),
            ConstraintToken::MintFreezeAuthority(c) => self.add_mint_freeze_authority(c),
            ConstraintToken::MintDecimals(c) => self.add_mint_decimals(c),
            ConstraintToken::MintTokenProgram(c) => self.add_mint_token_program(c),
            ConstraintToken::Bump(c) => self.add_bump(c),
            ConstraintToken::ProgramSeed(c) => self.add_program_seed(c),
            ConstraintToken::Realloc(c) => self.add_realloc(c),
            ConstraintToken::ReallocPayer(c) => self.add_realloc_payer(c),
            ConstraintToken::ReallocZero(c) => self.add_realloc_zero(c),
            ConstraintToken::ExtensionGroupPointerAuthority(c) => {
                self.add_extension_group_pointer_authority(c)
            }
            ConstraintToken::ExtensionGroupPointerGroupAddress(c) => {
                self.add_extension_group_pointer_group_address(c)
            }
            ConstraintToken::ExtensionGroupMemberPointerAuthority(c) => {
                self.add_extension_group_member_pointer_authority(c)
            }
            ConstraintToken::ExtensionGroupMemberPointerMemberAddress(c) => {
                self.add_extension_group_member_pointer_member_address(c)
            }
            ConstraintToken::ExtensionMetadataPointerAuthority(c) => {
                self.add_extension_metadata_pointer_authority(c)
            }
            ConstraintToken::ExtensionMetadataPointerMetadataAddress(c) => {
                self.add_extension_metadata_pointer_metadata_address(c)
            }
            ConstraintToken::ExtensionCloseAuthority(c) => self.add_extension_close_authority(c),
            ConstraintToken::ExtensionTokenHookAuthority(c) => self.add_extension_authority(c),
            ConstraintToken::ExtensionTokenHookProgramId(c) => {
                self.add_extension_transfer_hook_program_id(c)
            }
            ConstraintToken::ExtensionPermanentDelegate(c) => {
                self.add_extension_permanent_delegate(c)
            }
            ConstraintToken::Dup(c) => self.add_dup(c),
            ConstraintToken::ExtensionPausableAuthority(c) => {
                self.add_extension_pausable_authority(c)
            }
        }
    }

    fn add_init(&mut self, c: Context<ConstraintInit>) -> ParseResult<()> {
        if self.init.is_some() {
            return Err(ParseError::new(c.span(), "init already provided"));
        }
        if self.zeroed.is_some() {
            return Err(ParseError::new(c.span(), "zeroed already provided"));
        }
        if self.token_mint.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before token mint",
            ));
        }
        if self.token_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before token authority",
            ));
        }
        if self.token_token_program.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before token account token program",
            ));
        }
        if self.mint_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before mint authority",
            ));
        }
        if self.mint_freeze_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before mint freeze authority",
            ));
        }
        if self.mint_decimals.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before mint decimals",
            ));
        }
        if self.mint_token_program.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before mint token program",
            ));
        }
        if self.associated_token_mint.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before associated token mint",
            ));
        }
        if self.associated_token_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before associated token authority",
            ));
        }
        if self.associated_token_token_program.is_some() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before associated token account token program",
            ));
        }
        self.init.replace(c);
        Ok(())
    }

    fn add_zeroed(&mut self, c: Context<ConstraintZeroed>) -> ParseResult<()> {
        if self.zeroed.is_some() {
            return Err(ParseError::new(c.span(), "zeroed already provided"));
        }
        if self.init.is_some() {
            return Err(ParseError::new(c.span(), "init already provided"));
        }

        // Require a known account type that implements the `Discriminator` trait so that we can
        // get the discriminator length dynamically
        if !matches!(
            &self.f_ty,
            Some(Ty::Account(_) | Ty::LazyAccount(_) | Ty::AccountLoader(_))
        ) {
            return Err(ParseError::new(
                c.span(),
                "`zero` constraint requires the type to implement the `Discriminator` trait",
            ));
        }
        self.zeroed.replace(c);
        Ok(())
    }

    fn add_realloc(&mut self, c: Context<ConstraintRealloc>) -> ParseResult<()> {
        if !matches!(self.f_ty, Some(Ty::Account(_)))
            && !matches!(self.f_ty, Some(Ty::LazyAccount(_)))
            && !matches!(self.f_ty, Some(Ty::AccountLoader(_)))
            && !matches!(self.f_ty, Some(Ty::Migration(_)))
        {
            return Err(ParseError::new(
                c.span(),
                "realloc must be on an Account, LazyAccount, AccountLoader, or Migration",
            ));
        }
        if self.mutable.is_none() {
            return Err(ParseError::new(
                c.span(),
                "mut must be provided before realloc",
            ));
        }
        if self.realloc.is_some() {
            return Err(ParseError::new(c.span(), "realloc already provided"));
        }
        self.realloc.replace(c);
        Ok(())
    }

    fn add_realloc_payer(&mut self, c: Context<ConstraintReallocPayer>) -> ParseResult<()> {
        if self.realloc.is_none() {
            return Err(ParseError::new(
                c.span(),
                "realloc must be provided before realloc::payer",
            ));
        }
        if self.realloc_payer.is_some() {
            return Err(ParseError::new(c.span(), "realloc::payer already provided"));
        }
        self.realloc_payer.replace(c);
        Ok(())
    }

    fn add_realloc_zero(&mut self, c: Context<ConstraintReallocZero>) -> ParseResult<()> {
        if self.realloc.is_none() {
            return Err(ParseError::new(
                c.span(),
                "realloc must be provided before realloc::zero",
            ));
        }
        if self.realloc_zero.is_some() {
            return Err(ParseError::new(c.span(), "realloc::zero already provided"));
        }
        self.realloc_zero.replace(c);
        Ok(())
    }

    fn add_close(&mut self, c: Context<ConstraintClose>) -> ParseResult<()> {
        if !matches!(self.f_ty, Some(Ty::Account(_)))
            && !matches!(self.f_ty, Some(Ty::LazyAccount(_)))
            && !matches!(self.f_ty, Some(Ty::AccountLoader(_)))
        {
            return Err(ParseError::new(
                c.span(),
                "close must be on an Account, LazyAccount or AccountLoader",
            ));
        }
        if self.mutable.is_none() {
            return Err(ParseError::new(
                c.span(),
                "mut must be provided before close",
            ));
        }
        if self.close.is_some() {
            return Err(ParseError::new(c.span(), "close already provided"));
        }
        self.close.replace(c);
        Ok(())
    }

    fn add_address(&mut self, c: Context<ConstraintAddress>) -> ParseResult<()> {
        if self.address.is_some() {
            return Err(ParseError::new(c.span(), "address already provided"));
        }
        self.address.replace(c);
        Ok(())
    }

    fn add_token_mint(&mut self, c: Context<ConstraintTokenMint>) -> ParseResult<()> {
        if self.token_mint.is_some() {
            return Err(ParseError::new(c.span(), "token mint already provided"));
        }
        if self.associated_token_mint.is_some() {
            return Err(ParseError::new(
                c.span(),
                "associated token mint already provided",
            ));
        }
        self.token_mint.replace(c);
        Ok(())
    }

    fn add_associated_token_mint(&mut self, c: Context<ConstraintTokenMint>) -> ParseResult<()> {
        if self.associated_token_mint.is_some() {
            return Err(ParseError::new(
                c.span(),
                "associated token mint already provided",
            ));
        }
        if self.token_mint.is_some() {
            return Err(ParseError::new(c.span(), "token mint already provided"));
        }
        self.associated_token_mint.replace(c);
        Ok(())
    }

    fn add_bump(&mut self, c: Context<ConstraintTokenBump>) -> ParseResult<()> {
        if self.bump.is_some() {
            return Err(ParseError::new(c.span(), "bump already provided"));
        }
        if self.seeds.is_none() {
            return Err(ParseError::new(
                c.span(),
                "seeds must be provided before bump",
            ));
        }
        self.bump.replace(c);
        Ok(())
    }

    fn add_program_seed(&mut self, c: Context<ConstraintProgramSeed>) -> ParseResult<()> {
        if self.program_seed.is_some() {
            return Err(ParseError::new(c.span(), "seeds::program already provided"));
        }
        if self.seeds.is_none() {
            return Err(ParseError::new(
                c.span(),
                "seeds must be provided before seeds::program",
            ));
        }
        if let Some(ref init) = self.init {
            if init.if_needed {
                return Err(ParseError::new(
                    c.span(),
                    "seeds::program cannot be used with init_if_needed",
                ));
            } else {
                return Err(ParseError::new(
                    c.span(),
                    "seeds::program cannot be used with init",
                ));
            }
        }
        self.program_seed.replace(c);
        Ok(())
    }

    fn add_token_authority(&mut self, c: Context<ConstraintTokenAuthority>) -> ParseResult<()> {
        if self.token_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "token authority already provided",
            ));
        }
        self.token_authority.replace(c);
        Ok(())
    }

    fn add_associated_token_authority(
        &mut self,
        c: Context<ConstraintTokenAuthority>,
    ) -> ParseResult<()> {
        if self.associated_token_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "associated token authority already provided",
            ));
        }
        if self.token_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "token authority already provided",
            ));
        }
        self.associated_token_authority.replace(c);
        Ok(())
    }

    fn add_token_token_program(&mut self, c: Context<ConstraintTokenProgram>) -> ParseResult<()> {
        if self.token_token_program.is_some() {
            return Err(ParseError::new(
                c.span(),
                "token token_program already provided",
            ));
        }
        self.token_token_program.replace(c);
        Ok(())
    }

    fn add_associated_token_token_program(
        &mut self,
        c: Context<ConstraintTokenProgram>,
    ) -> ParseResult<()> {
        if self.associated_token_token_program.is_some() {
            return Err(ParseError::new(
                c.span(),
                "associated token token_program already provided",
            ));
        }
        self.associated_token_token_program.replace(c);
        Ok(())
    }

    fn add_mint_authority(&mut self, c: Context<ConstraintMintAuthority>) -> ParseResult<()> {
        if self.mint_authority.is_some() {
            return Err(ParseError::new(c.span(), "mint authority already provided"));
        }
        self.mint_authority.replace(c);
        Ok(())
    }

    fn add_mint_freeze_authority(
        &mut self,
        c: Context<ConstraintMintFreezeAuthority>,
    ) -> ParseResult<()> {
        if self.mint_freeze_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "mint freeze_authority already provided",
            ));
        }
        self.mint_freeze_authority.replace(c);
        Ok(())
    }

    fn add_mint_decimals(&mut self, c: Context<ConstraintMintDecimals>) -> ParseResult<()> {
        if self.mint_decimals.is_some() {
            return Err(ParseError::new(c.span(), "mint decimals already provided"));
        }
        self.mint_decimals.replace(c);
        Ok(())
    }

    fn add_mint_token_program(&mut self, c: Context<ConstraintTokenProgram>) -> ParseResult<()> {
        if self.mint_token_program.is_some() {
            return Err(ParseError::new(
                c.span(),
                "mint token_program already provided",
            ));
        }
        self.mint_token_program.replace(c);
        Ok(())
    }

    fn add_mut(&mut self, c: Context<ConstraintMut>) -> ParseResult<()> {
        if self.mutable.is_some() {
            return Err(ParseError::new(c.span(), "mut already provided"));
        }
        self.mutable.replace(c);
        Ok(())
    }

    fn add_signer(&mut self, c: Context<ConstraintSigner>) -> ParseResult<()> {
        if self.signer.is_some() {
            return Err(ParseError::new(c.span(), "signer already provided"));
        }
        self.signer.replace(c);
        Ok(())
    }

    fn add_has_one(&mut self, c: Context<ConstraintHasOne>) -> ParseResult<()> {
        if self
            .has_one
            .iter()
            .filter(|item| item.join_target == c.join_target)
            .count()
            > 0
        {
            return Err(ParseError::new(c.span(), "has_one target already provided"));
        }
        self.has_one.push(c);
        Ok(())
    }

    fn add_raw(&mut self, c: Context<ConstraintRaw>) -> ParseResult<()> {
        self.raw.push(c);
        Ok(())
    }

    fn add_owner(&mut self, c: Context<ConstraintOwner>) -> ParseResult<()> {
        if self.owner.is_some() {
            return Err(ParseError::new(c.span(), "owner already provided"));
        }
        self.owner.replace(c);
        Ok(())
    }

    fn add_rent_exempt(&mut self, c: Context<ConstraintRentExempt>) -> ParseResult<()> {
        if self.rent_exempt.is_some() {
            return Err(ParseError::new(c.span(), "rent already provided"));
        }
        self.rent_exempt.replace(c);
        Ok(())
    }

    fn add_seeds(&mut self, c: Context<ConstraintSeeds>) -> ParseResult<()> {
        if self.seeds.is_some() {
            return Err(ParseError::new(c.span(), "seeds already provided"));
        }
        self.seeds.replace(c);
        Ok(())
    }

    fn add_executable(&mut self, c: Context<ConstraintExecutable>) -> ParseResult<()> {
        if self.executable.is_some() {
            return Err(ParseError::new(c.span(), "executable already provided"));
        }
        self.executable.replace(c);
        Ok(())
    }

    fn add_payer(&mut self, c: Context<ConstraintPayer>) -> ParseResult<()> {
        if self.init.is_none() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before payer",
            ));
        }
        if self.payer.is_some() {
            return Err(ParseError::new(c.span(), "payer already provided"));
        }
        self.payer.replace(c);
        Ok(())
    }

    fn add_space(&mut self, c: Context<ConstraintSpace>) -> ParseResult<()> {
        if self.init.is_none() {
            return Err(ParseError::new(
                c.span(),
                "init must be provided before space",
            ));
        }
        if self.space.is_some() {
            return Err(ParseError::new(c.span(), "space already provided"));
        }
        self.space.replace(c);
        Ok(())
    }

    // extensions

    fn add_extension_group_pointer_authority(
        &mut self,
        c: Context<ConstraintExtensionAuthority>,
    ) -> ParseResult<()> {
        if self.extension_group_pointer_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension group pointer authority already provided",
            ));
        }
        self.extension_group_pointer_authority.replace(c);
        Ok(())
    }

    fn add_extension_group_pointer_group_address(
        &mut self,
        c: Context<ConstraintExtensionGroupPointerGroupAddress>,
    ) -> ParseResult<()> {
        if self.extension_group_pointer_group_address.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension group pointer group address already provided",
            ));
        }
        self.extension_group_pointer_group_address.replace(c);
        Ok(())
    }

    fn add_extension_group_member_pointer_authority(
        &mut self,
        c: Context<ConstraintExtensionAuthority>,
    ) -> ParseResult<()> {
        if self.extension_group_member_pointer_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension group member pointer authority already provided",
            ));
        }
        self.extension_group_member_pointer_authority.replace(c);
        Ok(())
    }

    fn add_extension_group_member_pointer_member_address(
        &mut self,
        c: Context<ConstraintExtensionGroupMemberPointerMemberAddress>,
    ) -> ParseResult<()> {
        if self.extension_group_member_pointer_member_address.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension group member pointer member address already provided",
            ));
        }
        self.extension_group_member_pointer_member_address
            .replace(c);
        Ok(())
    }

    fn add_extension_metadata_pointer_authority(
        &mut self,
        c: Context<ConstraintExtensionAuthority>,
    ) -> ParseResult<()> {
        if self.extension_metadata_pointer_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension metadata pointer authority already provided",
            ));
        }
        self.extension_metadata_pointer_authority.replace(c);
        Ok(())
    }

    fn add_extension_metadata_pointer_metadata_address(
        &mut self,
        c: Context<ConstraintExtensionMetadataPointerMetadataAddress>,
    ) -> ParseResult<()> {
        if self.extension_metadata_pointer_metadata_address.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension metadata pointer metadata address already provided",
            ));
        }
        self.extension_metadata_pointer_metadata_address.replace(c);
        Ok(())
    }

    fn add_extension_close_authority(
        &mut self,
        c: Context<ConstraintExtensionAuthority>,
    ) -> ParseResult<()> {
        if self.extension_close_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension close authority already provided",
            ));
        }
        self.extension_close_authority.replace(c);
        Ok(())
    }

    fn add_extension_authority(
        &mut self,
        c: Context<ConstraintExtensionAuthority>,
    ) -> ParseResult<()> {
        if self.extension_transfer_hook_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension transfer hook authority already provided",
            ));
        }
        self.extension_transfer_hook_authority.replace(c);
        Ok(())
    }

    fn add_extension_transfer_hook_program_id(
        &mut self,
        c: Context<ConstraintExtensionTokenHookProgramId>,
    ) -> ParseResult<()> {
        if self.extension_transfer_hook_program_id.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension transfer hook program id already provided",
            ));
        }
        self.extension_transfer_hook_program_id.replace(c);
        Ok(())
    }

    fn add_extension_permanent_delegate(
        &mut self,
        c: Context<ConstraintExtensionPermanentDelegate>,
    ) -> ParseResult<()> {
        if self.extension_permanent_delegate.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension permanent delegate already provided",
            ));
        }
        self.extension_permanent_delegate.replace(c);
        Ok(())
    }

    fn add_dup(&mut self, c: Context<ConstraintDup>) -> ParseResult<()> {
        if self.dup.is_some() {
            return Err(ParseError::new(c.span(), "dup already provided"));
        }
        self.dup.replace(c);
        Ok(())
    }

    fn add_extension_pausable_authority(
        &mut self,
        c: Context<ConstraintExtensionAuthority>,
    ) -> ParseResult<()> {
        if self.extension_pausable_authority.is_some() {
            return Err(ParseError::new(
                c.span(),
                "extension pausable authority already provided",
            ));
        }
        self.extension_pausable_authority.replace(c);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{parser::tts_to_string, ConstraintToken},
        syn::parse_str,
    };

    const LITERAL_ERROR: &str = "constraint must be a boolean expression, not a literal";
    const HELP_TEXT: &str = "constraint = my_field == <value>";

    fn parse_error_message(input: &str) -> Option<String> {
        parse_str::<ConstraintToken>(input)
            .err()
            .map(|error| error.to_string())
    }

    fn parse_raw_expression(input: &str) -> Option<String> {
        match parse_str::<ConstraintToken>(input) {
            Ok(ConstraintToken::Raw(raw)) => Some(tts_to_string(&raw.raw)),
            Ok(_) | Err(_) => None,
        }
    }

    #[test]
    fn rejects_string_literal_raw_constraint() {
        let message = parse_error_message(r#"constraint = "hello""#);
        assert!(message.is_some(), "expected parse error");
        let message = message.unwrap_or_default();

        assert!(
            message.contains(LITERAL_ERROR),
            "unexpected error: {message}"
        );
        assert!(message.contains(HELP_TEXT), "missing help text: {message}");
    }

    #[test]
    fn accepts_boolean_expression_that_compares_against_a_literal() {
        let expression = parse_raw_expression("constraint = my_field == 42");

        assert_eq!(expression.as_deref(), Some("my_field == 42"));
    }
}
