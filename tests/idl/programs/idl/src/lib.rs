use anchor_lang::prelude::*;
use anchor_spl::{token, token_interface};

declare_id!("id11111111111111111111111111111111111111111");

#[constant]
pub const U8: u8 = 6;

#[constant]
pub const I128: i128 = 1_000_000;

#[constant]
pub const BYTE_STR: u8 = b't';

#[constant]
pub const BYTES_STR: &[u8] = b"test";

pub const NO_IDL: u16 = 55;

#[program]
pub mod idl {
    use super::*;

    pub fn no_case_conversion(ctx: Context<NoCaseConversion>, field_name: u8) -> Result<()> {
        ctx.accounts.case_conversion_account.field_name = field_name;
        emit!(SimpleEvent { field_name });
        Ok(())
    }

    pub fn empty(_ctx: Context<Empty>) -> Result<()> {
        Ok(())
    }

    pub fn primitive_types(
        ctx: Context<PrimitiveTypes>,
        bool: bool,
        i8: i8,
        i16: i16,
        i32: i32,
        i64: i64,
        i128: i128,
        u8: u8,
        u16: u16,
        u32: u32,
        u64: u64,
        u128: u128,
        f32: f32,
        f64: f64,
        pubkey: Pubkey,
    ) -> Result<()> {
        ctx.accounts.account.bool = bool;

        ctx.accounts.account.i8 = i8;
        ctx.accounts.account.i16 = i16;
        ctx.accounts.account.i32 = i32;
        ctx.accounts.account.i64 = i64;
        ctx.accounts.account.i128 = i128;

        ctx.accounts.account.u8 = u8;
        ctx.accounts.account.u16 = u16;
        ctx.accounts.account.u32 = u32;
        ctx.accounts.account.u64 = u64;
        ctx.accounts.account.u128 = u128;

        ctx.accounts.account.f32 = f32;
        ctx.accounts.account.f64 = f64;

        ctx.accounts.account.pubkey = pubkey;
        Ok(())
    }

    pub fn unsized_types(ctx: Context<UnsizedTypes>, string: String, bytes: Vec<u8>) -> Result<()> {
        ctx.accounts.account.string = string;
        ctx.accounts.account.bytes = bytes;
        Ok(())
    }

    pub fn strct(
        ctx: Context<Struct>,
        unit: UnitStruct,
        named: NamedStruct,
        tuple: TupleStruct,
    ) -> Result<()> {
        ctx.accounts.account.unit = unit;
        ctx.accounts.account.named = named;
        ctx.accounts.account.tuple = tuple;
        Ok(())
    }

    pub fn enm(ctx: Context<Enum>, full_enum: FullEnum) -> Result<()> {
        ctx.accounts.account.full_enum = full_enum;
        Ok(())
    }

    pub fn type_alias(
        ctx: Context<TypeAlias>,
        alias_u8: AliasU8,
        alias_u8_array: AliasU8Array,
        alias_struct: AliasStruct,
        alias_vec_string: AliasVec<String>,
        alias_option_vec_pubkey: AliasOptionVec<Pubkey>,
        alias_generic_const: AliasGenericConst<4>,
        alias_multiple_generics_mixed: AliasMultipleGenericMixed<bool, 2>,
        alias_external: UnixTimestamp,
    ) -> Result<()> {
        ctx.accounts.account.alias_u8 = alias_u8;
        ctx.accounts.account.alias_u8_array = alias_u8_array;
        ctx.accounts.account.alias_struct = alias_struct;
        ctx.accounts.account.alias_vec_string = alias_vec_string;
        ctx.accounts.account.alias_option_vec_pubkey = alias_option_vec_pubkey;
        ctx.accounts.account.alias_generic_const = alias_generic_const;
        ctx.accounts.account.alias_multiple_generics_mixed = alias_multiple_generics_mixed;
        ctx.accounts.account.alias_external = alias_external;
        Ok(())
    }

    pub fn boxed(
        ctx: Context<Boxed>,
        u8: Box<u8>,
        u8_array: Box<[u8; 8]>,
        defined: Box<NamedStruct>,
        external: Box<external::MyStruct>,
        external_non_anchor: Box<wrapped::Feature>,
    ) -> Result<()> {
        ctx.accounts.account.u8 = u8;
        ctx.accounts.account.u8_array = u8_array;
        ctx.accounts.account.defined = defined;
        ctx.accounts.account.external = external;
        ctx.accounts.account.external_non_anchor = external_non_anchor;
        Ok(())
    }

    pub fn account_and_event_arg_and_field(
        ctx: Context<AccountAndEventArgAndField>,
        account: AccountAndEventFieldAccount,
    ) -> Result<()> {
        *ctx.accounts.account = account;
        Ok(())
    }

    pub fn generic(ctx: Context<Generic>, generic_arg: GenericStruct<u16, 4>) -> Result<()> {
        ctx.accounts.my_account.field = generic_arg;
        Ok(())
    }

    pub fn generic_custom_struct(
        ctx: Context<GenericCustomStruct>,
        generic_arg: GenericStruct<SomeStruct, 4>,
    ) -> Result<()> {
        ctx.accounts.my_account.field = generic_arg;
        Ok(())
    }

    pub fn full_path(
        ctx: Context<FullPath>,
        named_struct: NamedStruct,
        some_module_named_struct: some_module::NamedStruct,
    ) -> Result<()> {
        ctx.accounts.account.named_struct = named_struct;
        ctx.accounts.account.some_module_named_struct = some_module_named_struct;
        Ok(())
    }

    pub fn return_type(_ctx: Context<ReturnType>) -> Result<NamedStruct> {
        Ok(NamedStruct {
            u8: 1,
            u16: 2,
            u32: 4,
            u64: 8,
        })
    }

    pub fn external(ctx: Context<External>, my_struct: external::MyStruct) -> Result<()> {
        ctx.accounts.account.my_struct = my_struct;
        Ok(())
    }

    pub fn external_non_anchor(
        ctx: Context<ExternalNonAnchor>,
        feature: wrapped::Feature,
    ) -> Result<()> {
        ctx.accounts.account.feature = feature;
        Ok(())
    }

    pub fn test_compilation(_ctx: Context<TestCompilation>) -> Result<()> {
        Ok(())
    }
}

/// IDL test for the issue explained in https://github.com/otter-sec/anchor/issues/3358
///
/// For example, using [`SimpleAccount`] and adding the full path at the end of a doc comment
/// used to result in a false-positive when detecting conflicts.
///
/// [`SimpleAccount`]: crate::SimpleAccount
#[constant]
pub const TEST_CONVERT_MODULE_PATHS: &[u8] = b"convert_module_paths";

#[account]
#[derive(InitSpace)]
pub struct SimpleAccount {
    pub field_name: u8,
}

#[event]
#[derive(Clone)]
pub struct SimpleEvent {
    pub field_name: u8,
}

#[derive(Accounts)]
pub struct NoCaseConversion<'info> {
    #[account(init, payer = payer, space = 8 + SimpleAccount::INIT_SPACE)]
    pub case_conversion_account: Account<'info, SimpleAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct Empty {}

#[derive(Accounts)]
pub struct PrimitiveTypes<'info> {
    #[account(zero)]
    pub account: Account<'info, PrimitiveAccount>,
}

#[account]
pub struct PrimitiveAccount {
    pub bool: bool,
    pub i8: i8,
    pub i16: i16,
    pub i32: i32,
    pub i64: i64,
    pub i128: i128,
    pub u8: u8,
    pub u16: u16,
    pub u32: u32,
    pub u64: u64,
    pub u128: u128,
    pub f32: f32,
    pub f64: f64,
    pub pubkey: Pubkey,
}

#[derive(Accounts)]
pub struct UnsizedTypes<'info> {
    #[account(zero)]
    pub account: Account<'info, UnsizedAccount>,
}

#[account]
pub struct UnsizedAccount {
    pub string: String,
    pub bytes: Vec<u8>,
}

#[derive(Accounts)]
pub struct Struct<'info> {
    #[account(zero)]
    pub account: Account<'info, StructAccount>,
}

#[account]
pub struct StructAccount {
    pub unit: UnitStruct,
    pub named: NamedStruct,
    pub tuple: TupleStruct,
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone)]
pub struct UnitStruct;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamedStruct {
    pub u8: u8,
    pub u16: u16,
    pub u32: u32,
    pub u64: u64,
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone)]
pub struct TupleStruct(u64, String);

#[derive(Accounts)]
pub struct Enum<'info> {
    #[account(zero)]
    pub account: Account<'info, EnumAccount>,
}

#[account]
pub struct EnumAccount {
    pub full_enum: FullEnum,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum FullEnum {
    Unit,
    Named { point_x: u64, point_y: u64 },
    Unnamed(u8, u8, u16, u16),
    UnnamedStruct(NamedStruct),
}

#[derive(Accounts)]
pub struct TypeAlias<'info> {
    #[account(zero)]
    pub account: Account<'info, AliasAccount>,
}

#[account]
pub struct AliasAccount {
    pub alias_u8: AliasU8,
    pub alias_u8_array: AliasU8Array,
    pub alias_struct: AliasStruct,
    pub alias_vec_string: AliasVec<String>,
    pub alias_option_vec_pubkey: AliasOptionVec<Pubkey>,
    pub alias_generic_const: AliasGenericConst<4>,
    pub alias_multiple_generics_mixed: AliasMultipleGenericMixed<bool, 2>,
    pub alias_external: UnixTimestamp,
}

pub type AliasU8 = u8;
pub type AliasU8Array = [AliasU8; 8];
pub type AliasStruct = NamedStruct;
pub type AliasVec<T> = Vec<T>;
pub type AliasOptionVec<T> = Vec<Option<T>>;
pub type AliasGenericConst<const N: usize> = [u32; N];
pub type AliasMultipleGenericMixed<T, const N: usize> = Vec<[T; N]>;

// TODO: Remove this declaration and automatically resolve it from `solana-program`.
// Splitting `solana-program` into multiple parts in Solana v2.1 broke resolution of type
// aliases such as `UnixTimestamp` due to the resolution logic not being smart enough to figure
// out where the declaration of the type comes from.
pub type UnixTimestamp = i64;

#[derive(Accounts)]
pub struct Boxed<'info> {
    #[account(zero)]
    pub account: Account<'info, BoxedAccount>,
}

#[account]
pub struct BoxedAccount {
    pub u8: Box<u8>,
    pub u8_array: Box<[u8; 8]>,
    pub defined: Box<NamedStruct>,
    pub external: Box<external::MyStruct>,
    pub external_non_anchor: Box<wrapped::Feature>,
}

#[derive(Accounts)]
pub struct AccountAndEventArgAndField<'info> {
    #[account(zero)]
    pub account: Account<'info, AccountAndEventFieldAccount>,
}

#[account]
pub struct AccountAndEventFieldAccount {
    pub simple_account: SimpleAccount,
    pub simple_event: SimpleEvent,
}

#[derive(Accounts)]
pub struct FullPath<'info> {
    #[account(zero)]
    pub account: Account<'info, FullPathAccount>,
    pub external_program: Program<'info, external::program::External>,
}

#[account]
pub struct FullPathAccount {
    pub named_struct: NamedStruct,
    pub some_module_named_struct: some_module::NamedStruct,
}

mod some_module {
    use super::*;

    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct NamedStruct {
        pub data: u8,
    }
}

#[derive(Accounts)]
pub struct Generic<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        payer = signer,
        space = 1024,
        seeds = [b"generic", signer.key.as_ref()],
        bump
    )]
    pub my_account: Account<'info, GenericAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GenericCustomStruct<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        payer = signer,
        space = 1024,
        seeds = [b"genericCustomStruct", signer.key.as_ref()],
        bump
    )]
    pub my_account: Account<'info, GenericAccountCustomStruct>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct GenericAccount {
    pub field: GenericStruct<u16, 4>,
}

#[account]
pub struct GenericAccountCustomStruct {
    pub field: GenericStruct<SomeStruct, 4>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SomeStruct {
    pub field: u16,
}

/// Compilation check for the issue described in https://github.com/otter-sec/anchor/issues/3520
// TODO: Use this from client-side (instead of hardcoding) once `program.constants` is supported
const GENERIC_CONST: usize = 8;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GenericStruct<T: AnchorSerialize + AnchorDeserialize + Clone, const N: usize> {
    arr: [T; N],
    sub_field: SubGenericStruct<GENERIC_CONST, T, Vec<Option<T>>>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SubGenericStruct<
    const N: usize,
    T: AnchorSerialize + AnchorDeserialize + Clone,
    U: AnchorSerialize + AnchorDeserialize + Clone,
> {
    sub_arr: [T; N],
    another: U,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum GenericEnum<T: AnchorSerialize + AnchorDeserialize + Clone> {
    Unit,
    Named { x: T },
    Tuple(Vec<T>),
}

#[derive(Accounts)]
pub struct ReturnType {}

#[derive(Accounts)]
pub struct External<'info> {
    #[account(zero)]
    pub account: Account<'info, AccountWithExternalField>,
}

#[account]
pub struct AccountWithExternalField {
    pub my_struct: external::MyStruct,
}

#[derive(Accounts)]
pub struct ExternalNonAnchor<'info> {
    #[account(zero)]
    pub account: Account<'info, AccountWithNonAnchorExternalField>,
}

#[account]
pub struct AccountWithNonAnchorExternalField {
    pub feature: wrapped::Feature,
}

/// An example of wrapping a non-Anchor external type in order to include it in the IDL
mod wrapped {
    use super::*;

    #[cfg(feature = "idl-build")]
    use anchor_lang::idl::types::*;

    pub struct Feature(anchor_lang::solana_program::feature::Feature);

    impl AnchorSerialize for Feature {
        fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
            self.0.activated_at.serialize(writer)?;
            Ok(())
        }
    }

    impl AnchorDeserialize for Feature {
        fn deserialize_reader<R: std::io::prelude::Read>(reader: &mut R) -> std::io::Result<Self> {
            Ok(Self(anchor_lang::solana_program::feature::Feature {
                activated_at: AnchorDeserialize::deserialize_reader(reader)?,
            }))
        }
    }

    impl Clone for Feature {
        fn clone(&self) -> Self {
            Self(anchor_lang::solana_program::feature::Feature {
                activated_at: self.0.activated_at.clone(),
            })
        }
    }

    #[cfg(feature = "idl-build")]
    impl IdlBuild for Feature {
        fn create_type() -> Option<IdlTypeDef> {
            Some(IdlTypeDef {
                name: "Feature".into(),
                ty: IdlTypeDefTy::Struct {
                    fields: Some(IdlDefinedFields::Named(vec![IdlField {
                        name: "activated_at".into(),
                        ty: IdlType::Option(Box::new(IdlType::U64)),
                        docs: Default::default(),
                    }])),
                },
                docs: Default::default(),
                generics: Default::default(),
                serialization: Default::default(),
                repr: Default::default(),
            })
        }
    }
}

#[derive(Accounts)]
pub struct TestCompilation<'info> {
    account: Account<'info, SimpleAccount>,
    zc_account: AccountLoader<'info, ZcAccount>,
    zc_unsafe_account: AccountLoader<'info, ZcUnsafeAccount>,

    composite: Composite<'info>,

    external: Account<'info, external::MyAccount>,

    token_account: Account<'info, token::TokenAccount>,
    mint_account: Account<'info, token::Mint>,
    token_interface_account: InterfaceAccount<'info, token_interface::TokenAccount>,
    mint_interface_account: InterfaceAccount<'info, token_interface::Mint>,

    system_program: Program<'info, System>,
    token_program: Program<'info, token::Token>,
    token_interface: Interface<'info, token_interface::TokenInterface>,

    clock_sysvar: Sysvar<'info, Clock>,
    rent_sysvar: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Composite<'info> {
    account: Account<'info, SimpleAccount>,
}

#[account(zero_copy)]
pub struct ZcAccount {}

#[account(zero_copy(unsafe))]
pub struct ZcUnsafeAccount {}

#[error_code(offset = 500_000)]
pub enum ErrorCode {
    #[msg("Example error.")]
    SomeError,
    #[msg("Another error.")]
    OtherError,
    ErrorWithoutMsg,
    WithDiscrim = 500,
}
