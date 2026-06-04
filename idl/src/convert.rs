//! IDL spec conversion.
//!
//! Two public entry points, one per direction:
//! - [`convert_idl`]: legacy (pre Anchor v0.30) -> current spec.
//! - [`convert_idl_to_legacy`]: current spec -> legacy.
//!
//! The legacy spec types and both `impl` directions live inside the
//! private [`legacy`] module so that all of the schema's quirks stay in
//! one place.

use {
    crate::types::Idl,
    anyhow::{anyhow, Result},
};

/// Create an [`Idl`] value with additional support for older specs based on the
/// `idl.metadata.spec` field.
///
/// If `spec` field is not specified, the conversion will fallback to the legacy IDL spec
/// (pre Anchor v0.30).
///
/// **Note:** For legacy IDLs, `idl.metadata.address` field is required to be populated with
/// program's address otherwise an error will be returned.
pub fn convert_idl(idl: &[u8]) -> Result<Idl> {
    let value = serde_json::from_slice::<serde_json::Value>(idl)?;
    let spec = value
        .get("metadata")
        .and_then(|m| m.get("spec"))
        .and_then(|spec| spec.as_str());
    match spec {
        // New standard
        Some(spec) => match spec {
            "0.1.0" => serde_json::from_value(value).map_err(Into::into),
            _ => Err(anyhow!("IDL spec not supported: `{spec}`")),
        },
        // Legacy
        None => serde_json::from_value::<legacy::Idl>(value).map(TryInto::try_into)?,
    }
}

/// Convert a current-spec [`Idl`] into the legacy (pre-Anchor v0.30) IDL
/// representation and return its pretty-printed JSON bytes.
pub fn convert_idl_to_legacy(idl: &Idl) -> Result<Vec<u8>> {
    let legacy_idl = legacy::to_legacy(idl)?;
    serde_json::to_vec_pretty(&legacy_idl).map_err(Into::into)
}

/// Legacy IDL spec (pre Anchor v0.30).
///
/// Contains three things, in order:
/// 1. The legacy schema types (`Idl`, `IdlInstruction`, ...).
/// 2. `legacy -> current` conversions (`impl From<...> for t::...`).
/// 3. `current -> legacy` conversions (`impl TryFrom<t::...> for ...`)
///    plus the entry point [`to_legacy`].
mod legacy {
    use {
        crate::types as t,
        anyhow::{anyhow, Result},
        heck::{MixedCase, SnakeCase},
        serde::{Deserialize, Serialize},
    };

    // ---------------------------------------------------------------------
    // Legacy spec types.
    // ---------------------------------------------------------------------

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Idl {
        pub version: String,
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub docs: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub constants: Vec<IdlConst>,
        pub instructions: Vec<IdlInstruction>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub accounts: Vec<IdlTypeDefinition>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub types: Vec<IdlTypeDefinition>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub events: Option<Vec<IdlEvent>>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub errors: Option<Vec<IdlErrorCode>>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub metadata: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlConst {
        pub name: String,
        #[serde(rename = "type")]
        pub ty: IdlType,
        pub value: String,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlState {
        #[serde(rename = "struct")]
        pub strct: IdlTypeDefinition,
        pub methods: Vec<IdlInstruction>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlInstruction {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub docs: Option<Vec<String>>,
        pub accounts: Vec<IdlAccountItem>,
        pub args: Vec<IdlField>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub returns: Option<IdlType>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct IdlAccounts {
        pub name: String,
        pub accounts: Vec<IdlAccountItem>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(untagged)]
    pub enum IdlAccountItem {
        IdlAccount(IdlAccount),
        IdlAccounts(IdlAccounts),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct IdlAccount {
        pub name: String,
        pub is_mut: bool,
        pub is_signer: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub is_optional: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub docs: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub pda: Option<IdlPda>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub relations: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct IdlPda {
        pub seeds: Vec<IdlSeed>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub program_id: Option<IdlSeed>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase", tag = "kind")]
    pub enum IdlSeed {
        Const(IdlSeedConst),
        Arg(IdlSeedArg),
        Account(IdlSeedAccount),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct IdlSeedAccount {
        #[serde(rename = "type")]
        pub ty: IdlType,
        // account_ty points to the entry in the "accounts" section.
        // Some only if the `Account<T>` type is used.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub account: Option<String>,
        pub path: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct IdlSeedArg {
        #[serde(rename = "type")]
        pub ty: IdlType,
        pub path: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct IdlSeedConst {
        #[serde(rename = "type")]
        pub ty: IdlType,
        pub value: serde_json::Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlField {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub docs: Option<Vec<String>>,
        #[serde(rename = "type")]
        pub ty: IdlType,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlEvent {
        pub name: String,
        pub fields: Vec<IdlEventField>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlEventField {
        pub name: String,
        #[serde(rename = "type")]
        pub ty: IdlType,
        pub index: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlTypeDefinition {
        /// - `idl-parse`: always the name of the type
        /// - `idl-build`: full path if there is a name conflict, otherwise the name of the type
        pub name: String,
        /// Documentation comments
        #[serde(skip_serializing_if = "Option::is_none")]
        pub docs: Option<Vec<String>>,
        /// Generics, only supported with `idl-build`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub generics: Option<Vec<String>>,
        /// Type definition, `struct` or `enum`
        #[serde(rename = "type")]
        pub ty: IdlTypeDefinitionTy,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "lowercase", tag = "kind")]
    pub enum IdlTypeDefinitionTy {
        Struct { fields: Vec<IdlField> },
        Enum { variants: Vec<IdlEnumVariant> },
        Alias { value: IdlType },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlEnumVariant {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub fields: Option<EnumFields>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(untagged)]
    pub enum EnumFields {
        Named(Vec<IdlField>),
        Tuple(Vec<IdlType>),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub enum IdlType {
        Bool,
        U8,
        I8,
        U16,
        I16,
        U32,
        I32,
        F32,
        U64,
        I64,
        F64,
        U128,
        I128,
        U256,
        I256,
        Bytes,
        String,
        PublicKey,
        Defined(String),
        Option(Box<IdlType>),
        Vec(Box<IdlType>),
        Array(Box<IdlType>, usize),
        GenericLenArray(Box<IdlType>, String),
        Generic(String),
        DefinedWithTypeArgs {
            name: String,
            args: Vec<IdlDefinedTypeArg>,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub enum IdlDefinedTypeArg {
        Generic(String),
        Value(String),
        Type(IdlType),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct IdlErrorCode {
        pub code: u32,
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub msg: Option<String>,
    }

    /// Apply `transform` to each dot-separated segment of an identifier
    /// path (e.g. `acc.field` or `"authority"`). Used for `relations`
    /// entries and PDA seed paths, which reference other accounts or
    /// instruction args and so must follow the same casing convention.
    fn recase_path(path: &str, transform: impl Fn(&str) -> String) -> String {
        path.split('.').map(transform).collect::<Vec<_>>().join(".")
    }

    // ---------------------------------------------------------------------
    // Forward conversion: legacy (`Idl`) -> current spec (`t::Idl`).
    // ---------------------------------------------------------------------

    impl TryFrom<Idl> for t::Idl {
        type Error = anyhow::Error;

        fn try_from(idl: Idl) -> Result<Self> {
            Ok(Self {
                address: idl
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("address"))
                    .and_then(|a| a.as_str())
                    .ok_or_else(|| anyhow!("Program id missing in `idl.metadata.address` field"))?
                    .into(),
                metadata: t::IdlMetadata {
                    name: idl.name,
                    version: idl.version,
                    spec: t::IDL_SPEC.into(),
                    description: Default::default(),
                    repository: Default::default(),
                    dependencies: Default::default(),
                    contact: Default::default(),
                    deployments: Default::default(),
                },
                docs: idl.docs.unwrap_or_default(),
                instructions: idl
                    .instructions
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_>>()?,
                accounts: idl.accounts.clone().into_iter().map(Into::into).collect(),
                events: idl
                    .events
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                errors: idl
                    .errors
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                types: idl
                    .types
                    .into_iter()
                    .map(Into::into)
                    .chain(idl.accounts.into_iter().map(Into::into))
                    .chain(idl.events.unwrap_or_default().into_iter().map(Into::into))
                    .collect(),
                constants: idl.constants.into_iter().map(Into::into).collect(),
            })
        }
    }

    fn get_disc(prefix: &str, name: &str) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(prefix);
        hasher.update(b":");
        hasher.update(name);
        hasher.finalize()[..8].into()
    }

    impl TryFrom<IdlInstruction> for t::IdlInstruction {
        type Error = anyhow::Error;

        fn try_from(value: IdlInstruction) -> Result<Self> {
            let name = value.name.to_snake_case();
            Ok(Self {
                discriminator: get_disc("global", &name),
                name,
                docs: value.docs.unwrap_or_default(),
                accounts: value
                    .accounts
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_>>()?,
                args: value.args.into_iter().map(Into::into).collect(),
                returns: value.returns.map(|r| r.into()),
            })
        }
    }

    impl From<IdlTypeDefinition> for t::IdlAccount {
        fn from(value: IdlTypeDefinition) -> Self {
            Self {
                discriminator: get_disc("account", &value.name),
                name: value.name,
            }
        }
    }

    impl From<IdlEvent> for t::IdlEvent {
        fn from(value: IdlEvent) -> Self {
            Self {
                discriminator: get_disc("event", &value.name),
                name: value.name,
            }
        }
    }

    impl From<IdlErrorCode> for t::IdlErrorCode {
        fn from(value: IdlErrorCode) -> Self {
            Self {
                name: value.name,
                code: value.code,
                msg: value.msg,
            }
        }
    }

    impl From<IdlConst> for t::IdlConst {
        fn from(value: IdlConst) -> Self {
            Self {
                name: value.name,
                docs: Default::default(),
                ty: value.ty.into(),
                value: value.value,
            }
        }
    }

    impl From<IdlDefinedTypeArg> for t::IdlGenericArg {
        fn from(value: IdlDefinedTypeArg) -> Self {
            match value {
                IdlDefinedTypeArg::Type(ty) => Self::Type { ty: ty.into() },
                IdlDefinedTypeArg::Value(value) => Self::Const { value },
                IdlDefinedTypeArg::Generic(generic) => Self::Type {
                    ty: t::IdlType::Generic(generic),
                },
            }
        }
    }

    impl From<IdlTypeDefinition> for t::IdlTypeDef {
        fn from(value: IdlTypeDefinition) -> Self {
            Self {
                name: value.name,
                docs: value.docs.unwrap_or_default(),
                serialization: Default::default(),
                repr: Default::default(),
                generics: Default::default(),
                ty: value.ty.into(),
            }
        }
    }

    impl From<IdlEvent> for t::IdlTypeDef {
        fn from(value: IdlEvent) -> Self {
            Self {
                name: value.name,
                docs: Default::default(),
                serialization: Default::default(),
                repr: Default::default(),
                generics: Default::default(),
                ty: t::IdlTypeDefTy::Struct {
                    fields: Some(t::IdlDefinedFields::Named(
                        value
                            .fields
                            .into_iter()
                            .map(|f| t::IdlField {
                                name: f.name.to_snake_case(),
                                docs: Default::default(),
                                ty: f.ty.into(),
                            })
                            .collect(),
                    )),
                },
            }
        }
    }

    impl From<IdlTypeDefinitionTy> for t::IdlTypeDefTy {
        fn from(value: IdlTypeDefinitionTy) -> Self {
            match value {
                IdlTypeDefinitionTy::Struct { fields } => Self::Struct {
                    fields: if fields.is_empty() {
                        None
                    } else {
                        Some(t::IdlDefinedFields::Named(
                            fields.into_iter().map(Into::into).collect(),
                        ))
                    },
                },
                IdlTypeDefinitionTy::Enum { variants } => Self::Enum {
                    variants: variants
                        .into_iter()
                        .map(|variant| t::IdlEnumVariant {
                            name: variant.name,
                            fields: variant.fields.map(|fields| match fields {
                                EnumFields::Named(fields) => t::IdlDefinedFields::Named(
                                    fields.into_iter().map(Into::into).collect(),
                                ),
                                EnumFields::Tuple(tys) => t::IdlDefinedFields::Tuple(
                                    tys.into_iter().map(Into::into).collect(),
                                ),
                            }),
                        })
                        .collect(),
                },
                IdlTypeDefinitionTy::Alias { value } => Self::Type {
                    alias: value.into(),
                },
            }
        }
    }

    impl From<IdlField> for t::IdlField {
        fn from(value: IdlField) -> Self {
            Self {
                name: value.name.to_snake_case(),
                docs: value.docs.unwrap_or_default(),
                ty: value.ty.into(),
            }
        }
    }

    impl From<IdlType> for t::IdlType {
        fn from(value: IdlType) -> Self {
            match value {
                IdlType::PublicKey => t::IdlType::Pubkey,
                IdlType::Defined(name) => t::IdlType::Defined {
                    name,
                    generics: Default::default(),
                },
                IdlType::DefinedWithTypeArgs { name, args } => t::IdlType::Defined {
                    name,
                    generics: args.into_iter().map(Into::into).collect(),
                },
                IdlType::Option(ty) => t::IdlType::Option(ty.into()),
                IdlType::Vec(ty) => t::IdlType::Vec(ty.into()),
                IdlType::Array(ty, len) => t::IdlType::Array(ty.into(), t::IdlArrayLen::Value(len)),
                IdlType::GenericLenArray(ty, generic) => {
                    t::IdlType::Array(ty.into(), t::IdlArrayLen::Generic(generic))
                }
                _ => serde_json::to_value(value)
                    .and_then(serde_json::from_value)
                    .unwrap(),
            }
        }
    }

    impl From<Box<IdlType>> for Box<t::IdlType> {
        fn from(value: Box<IdlType>) -> Self {
            Box::new((*value).into())
        }
    }

    impl TryFrom<IdlAccountItem> for t::IdlInstructionAccountItem {
        type Error = anyhow::Error;

        fn try_from(value: IdlAccountItem) -> Result<Self> {
            Ok(match value {
                IdlAccountItem::IdlAccount(acc) => Self::Single(t::IdlInstructionAccount {
                    name: acc.name.to_snake_case(),
                    docs: acc.docs.unwrap_or_default(),
                    writable: acc.is_mut,
                    signer: acc.is_signer,
                    optional: acc.is_optional.unwrap_or_default(),
                    address: Default::default(),
                    pda: acc
                        .pda
                        .map(|pda| -> Result<t::IdlPda> {
                            Ok(t::IdlPda {
                                seeds: pda
                                    .seeds
                                    .into_iter()
                                    .map(TryInto::try_into)
                                    .collect::<Result<_>>()?,
                                program: pda.program_id.map(TryInto::try_into).transpose()?,
                            })
                        })
                        .transpose()?,
                    relations: acc
                        .relations
                        .into_iter()
                        .map(|r| recase_path(&r, |s| s.to_snake_case()))
                        .collect(),
                }),
                IdlAccountItem::IdlAccounts(accs) => Self::Composite(t::IdlInstructionAccounts {
                    name: accs.name.to_snake_case(),
                    accounts: accs
                        .accounts
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<_>>()?,
                }),
            })
        }
    }

    impl TryFrom<IdlSeed> for t::IdlSeed {
        type Error = anyhow::Error;

        fn try_from(value: IdlSeed) -> Result<Self> {
            let seed = match value {
                IdlSeed::Account(seed) => Self::Account(t::IdlSeedAccount {
                    // `account` is a type name (PascalCase) — leave it.
                    account: seed.account,
                    // `path` references an account in the same instruction
                    // and must follow the snake_case account naming.
                    path: recase_path(&seed.path, |s| s.to_snake_case()),
                }),
                IdlSeed::Arg(seed) => Self::Arg(t::IdlSeedArg {
                    path: recase_path(&seed.path, |s| s.to_snake_case()),
                }),
                IdlSeed::Const(seed) => Self::Const(t::IdlSeedConst {
                    value: match seed.ty {
                        IdlType::String => seed.value.to_string().as_bytes().into(),
                        // The inverse converter emits const seeds as
                        // `type: "bytes"` + JSON array of u8 values, so
                        // accept that shape too. Lets `current -> legacy
                        // -> current` round-trip preserve PDA const seeds.
                        IdlType::Bytes => {
                            let arr = seed.value.as_array().ok_or_else(|| {
                                anyhow!("Const seed of type `bytes` must be a JSON array")
                            })?;
                            arr.iter()
                                .map(|v| {
                                    v.as_u64()
                                        .and_then(|n| u8::try_from(n).ok())
                                        .ok_or_else(|| {
                                            anyhow!("Const seed bytes must be u8 values, got {v}")
                                        })
                                })
                                .collect::<Result<Vec<u8>>>()?
                        }
                        _ => {
                            return Err(anyhow!(
                                "Const seed conversion not supported for type {:?}",
                                seed.ty
                            ))
                        }
                    },
                }),
            };
            Ok(seed)
        }
    }

    // ---------------------------------------------------------------------
    // Inverse conversion: current spec (`t::Idl`) -> legacy (`Idl`).
    // ---------------------------------------------------------------------

    /// Build a legacy [`Idl`] from a current-spec [`t::Idl`]. The lossy
    /// rules (dropped fields, defaulted seed types, hard errors) are
    /// documented on the public [`super::convert_idl_to_legacy`] entry.
    pub(super) fn to_legacy(idl: &t::Idl) -> Result<Idl> {
        use std::collections::{HashMap, HashSet};

        // Types listed under `accounts` and `events` get re-inlined into the
        // legacy `accounts`/`events` arrays, so we remove them from the top
        // level `types` array to avoid duplication.
        let account_names: HashSet<&str> = idl.accounts.iter().map(|a| a.name.as_str()).collect();
        let event_names: HashSet<&str> = idl.events.iter().map(|e| e.name.as_str()).collect();
        let td_by_name: HashMap<&str, &t::IdlTypeDef> =
            idl.types.iter().map(|td| (td.name.as_str(), td)).collect();

        let accounts = idl
            .accounts
            .iter()
            .map(|acc| {
                let td = td_by_name.get(acc.name.as_str()).ok_or_else(|| {
                    anyhow!(
                        "Account `{}` is referenced in `accounts` but not defined in `types`",
                        acc.name
                    )
                })?;
                IdlTypeDefinition::try_from((*td).clone())
            })
            .collect::<Result<Vec<_>>>()?;

        let events: Vec<IdlEvent> = idl
            .events
            .iter()
            .map(|evt| {
                let td = td_by_name.get(evt.name.as_str()).ok_or_else(|| {
                    anyhow!(
                        "Event `{}` is referenced in `events` but not defined in `types`",
                        evt.name
                    )
                })?;
                event_from_typedef((*td).clone())
            })
            .collect::<Result<_>>()?;

        let types: Vec<IdlTypeDefinition> = idl
            .types
            .iter()
            .filter(|td| {
                !account_names.contains(td.name.as_str()) && !event_names.contains(td.name.as_str())
            })
            .cloned()
            .map(IdlTypeDefinition::try_from)
            .collect::<Result<_>>()?;

        Ok(Idl {
            version: idl.metadata.version.clone(),
            name: idl.metadata.name.clone(),
            docs: (!idl.docs.is_empty()).then(|| idl.docs.clone()),
            constants: idl
                .constants
                .iter()
                .cloned()
                .map(IdlConst::try_from)
                .collect::<Result<_>>()?,
            instructions: idl
                .instructions
                .iter()
                .cloned()
                .map(IdlInstruction::try_from)
                .collect::<Result<_>>()?,
            accounts,
            types,
            events: (!events.is_empty()).then_some(events),
            errors: (!idl.errors.is_empty())
                .then(|| idl.errors.iter().cloned().map(Into::into).collect()),
            metadata: Some(serde_json::json!({ "address": idl.address })),
        })
    }

    /// Reconstruct a legacy [`IdlEvent`] from the named-struct type that
    /// the current spec stores under `types`. The legacy `index: bool`
    /// flag has no counterpart and is defaulted to `false`.
    fn event_from_typedef(td: t::IdlTypeDef) -> Result<IdlEvent> {
        let name = td.name;
        match td.ty {
            t::IdlTypeDefTy::Struct { fields } => match fields {
                Some(t::IdlDefinedFields::Named(fs)) => Ok(IdlEvent {
                    name,
                    fields: fs
                        .into_iter()
                        .map(|f| {
                            Ok(IdlEventField {
                                // Legacy event fields use lowerCamelCase.
                                name: f.name.to_mixed_case(),
                                ty: f.ty.try_into()?,
                                // The `index` flag was removed in the current
                                // spec and cannot be recovered when going back
                                // to the legacy representation.
                                index: false,
                            })
                        })
                        .collect::<Result<_>>()?,
                }),
                None => Ok(IdlEvent {
                    name,
                    fields: vec![],
                }),
                Some(t::IdlDefinedFields::Tuple(_)) => Err(anyhow!(
                    "Event `{name}` is a tuple struct; legacy IDLs require named fields"
                )),
            },
            _ => Err(anyhow!(
                "Event `{name}` must be defined as a struct in the current spec"
            )),
        }
    }

    impl TryFrom<t::IdlInstruction> for IdlInstruction {
        type Error = anyhow::Error;

        fn try_from(ix: t::IdlInstruction) -> Result<Self> {
            Ok(Self {
                // The legacy spec uses lowerCamelCase identifiers; the forward
                // converter normalized them to snake_case, so we restore the
                // original convention here.
                name: ix.name.to_mixed_case(),
                docs: (!ix.docs.is_empty()).then_some(ix.docs),
                accounts: ix.accounts.into_iter().map(Into::into).collect(),
                args: ix
                    .args
                    .into_iter()
                    .map(IdlField::try_from)
                    .collect::<Result<_>>()?,
                returns: ix.returns.map(IdlType::try_from).transpose()?,
            })
        }
    }

    impl From<t::IdlInstructionAccountItem> for IdlAccountItem {
        fn from(item: t::IdlInstructionAccountItem) -> Self {
            match item {
                t::IdlInstructionAccountItem::Single(acc) => Self::IdlAccount(IdlAccount {
                    // Legacy account names are lowerCamelCase.
                    name: acc.name.to_mixed_case(),
                    is_mut: acc.writable,
                    is_signer: acc.signer,
                    is_optional: acc.optional.then_some(true),
                    docs: (!acc.docs.is_empty()).then_some(acc.docs),
                    pda: acc.pda.map(Into::into),
                    relations: acc
                        .relations
                        .into_iter()
                        .map(|r| recase_path(&r, |s| s.to_mixed_case()))
                        .collect(),
                }),
                t::IdlInstructionAccountItem::Composite(accs) => Self::IdlAccounts(IdlAccounts {
                    name: accs.name.to_mixed_case(),
                    accounts: accs.accounts.into_iter().map(Into::into).collect(),
                }),
            }
        }
    }

    impl From<t::IdlPda> for IdlPda {
        fn from(p: t::IdlPda) -> Self {
            Self {
                seeds: p.seeds.into_iter().map(Into::into).collect(),
                program_id: p.program.map(Into::into),
            }
        }
    }

    impl From<t::IdlSeed> for IdlSeed {
        fn from(s: t::IdlSeed) -> Self {
            match s {
                t::IdlSeed::Const(c) => Self::Const(IdlSeedConst {
                    // The current spec stores const seeds as raw bytes;
                    // legacy requires a typed value. Bytes is the safe
                    // default that round-trips without information loss.
                    ty: IdlType::Bytes,
                    value: serde_json::Value::Array(
                        c.value
                            .into_iter()
                            .map(|b| serde_json::Value::Number(b.into()))
                            .collect(),
                    ),
                }),
                t::IdlSeed::Arg(a) => Self::Arg(IdlSeedArg {
                    // The current spec dropped the seed argument's declared
                    // type. `Bytes` is a placeholder; downstream code that
                    // requires accurate seed types must read the original
                    // legacy IDL.
                    ty: IdlType::Bytes,
                    path: recase_path(&a.path, |s| s.to_mixed_case()),
                }),
                t::IdlSeed::Account(a) => Self::Account(IdlSeedAccount {
                    // Account-derived seeds are always pubkeys.
                    ty: IdlType::PublicKey,
                    // `account` is a type name (PascalCase) — leave it.
                    account: a.account,
                    // `path` references an account in the same instruction
                    // and must follow legacy lowerCamelCase naming.
                    path: recase_path(&a.path, |s| s.to_mixed_case()),
                }),
            }
        }
    }

    impl TryFrom<t::IdlField> for IdlField {
        type Error = anyhow::Error;

        fn try_from(f: t::IdlField) -> Result<Self> {
            Ok(Self {
                // Legacy struct fields, instruction args, and event fields
                // all use lowerCamelCase.
                name: f.name.to_mixed_case(),
                docs: (!f.docs.is_empty()).then_some(f.docs),
                ty: f.ty.try_into()?,
            })
        }
    }

    impl TryFrom<t::IdlConst> for IdlConst {
        type Error = anyhow::Error;

        fn try_from(c: t::IdlConst) -> Result<Self> {
            Ok(Self {
                name: c.name,
                ty: c.ty.try_into()?,
                value: c.value,
            })
        }
    }

    impl From<t::IdlErrorCode> for IdlErrorCode {
        fn from(e: t::IdlErrorCode) -> Self {
            Self {
                code: e.code,
                name: e.name,
                msg: e.msg,
            }
        }
    }

    impl TryFrom<t::IdlTypeDef> for IdlTypeDefinition {
        type Error = anyhow::Error;

        fn try_from(td: t::IdlTypeDef) -> Result<Self> {
            let name = td.name;
            if !matches!(td.serialization, t::IdlSerialization::Borsh) {
                return Err(anyhow!(
                    "Type `{name}` uses non-Borsh serialization ({:?}); the legacy IDL spec only \
                     supports Borsh",
                    td.serialization
                ));
            }
            let generics = if td.generics.is_empty() {
                None
            } else {
                Some(
                    td.generics
                        .into_iter()
                        .map(|g| match g {
                            t::IdlTypeDefGeneric::Type { name: g_name } => Ok(g_name),
                            t::IdlTypeDefGeneric::Const { .. } => Err(anyhow!(
                                "Type `{name}` uses const generics; the legacy IDL spec only \
                                 supports type generics"
                            )),
                        })
                        .collect::<Result<Vec<_>>>()?,
                )
            };
            Ok(Self {
                name,
                docs: (!td.docs.is_empty()).then_some(td.docs),
                generics,
                ty: td.ty.try_into()?,
            })
        }
    }

    impl TryFrom<t::IdlTypeDefTy> for IdlTypeDefinitionTy {
        type Error = anyhow::Error;

        fn try_from(ty: t::IdlTypeDefTy) -> Result<Self> {
            Ok(match ty {
                t::IdlTypeDefTy::Struct { fields } => Self::Struct {
                    fields: match fields {
                        None => vec![],
                        Some(t::IdlDefinedFields::Named(fs)) => fs
                            .into_iter()
                            .map(IdlField::try_from)
                            .collect::<Result<_>>()?,
                        Some(t::IdlDefinedFields::Tuple(_)) => {
                            return Err(anyhow!(
                                "Tuple-style struct fields have no legacy IDL equivalent"
                            ))
                        }
                    },
                },
                t::IdlTypeDefTy::Enum { variants } => Self::Enum {
                    variants: variants
                        .into_iter()
                        .map(|v| -> Result<IdlEnumVariant> {
                            Ok(IdlEnumVariant {
                                name: v.name,
                                fields: v
                                    .fields
                                    .map(|f| -> Result<EnumFields> {
                                        Ok(match f {
                                            t::IdlDefinedFields::Named(fs) => EnumFields::Named(
                                                fs.into_iter()
                                                    .map(IdlField::try_from)
                                                    .collect::<Result<_>>()?,
                                            ),
                                            t::IdlDefinedFields::Tuple(tys) => EnumFields::Tuple(
                                                tys.into_iter()
                                                    .map(IdlType::try_from)
                                                    .collect::<Result<_>>()?,
                                            ),
                                        })
                                    })
                                    .transpose()?,
                            })
                        })
                        .collect::<Result<_>>()?,
                },
                t::IdlTypeDefTy::Type { alias } => Self::Alias {
                    value: alias.try_into()?,
                },
            })
        }
    }

    impl TryFrom<t::IdlType> for IdlType {
        type Error = anyhow::Error;

        fn try_from(ty: t::IdlType) -> Result<Self> {
            Ok(match ty {
                t::IdlType::Bool => Self::Bool,
                t::IdlType::U8 => Self::U8,
                t::IdlType::I8 => Self::I8,
                t::IdlType::U16 => Self::U16,
                t::IdlType::I16 => Self::I16,
                t::IdlType::U32 => Self::U32,
                t::IdlType::I32 => Self::I32,
                t::IdlType::F32 => Self::F32,
                t::IdlType::U64 => Self::U64,
                t::IdlType::I64 => Self::I64,
                t::IdlType::F64 => Self::F64,
                t::IdlType::U128 => Self::U128,
                t::IdlType::I128 => Self::I128,
                t::IdlType::U256 => Self::U256,
                t::IdlType::I256 => Self::I256,
                t::IdlType::Bytes => Self::Bytes,
                t::IdlType::String => Self::String,
                t::IdlType::Pubkey => Self::PublicKey,
                t::IdlType::Option(inner) => Self::Option(Box::new((*inner).try_into()?)),
                t::IdlType::Vec(inner) => Self::Vec(Box::new((*inner).try_into()?)),
                t::IdlType::Array(inner, len) => {
                    let inner = Box::new((*inner).try_into()?);
                    match len {
                        t::IdlArrayLen::Value(n) => Self::Array(inner, n),
                        t::IdlArrayLen::Generic(name) => Self::GenericLenArray(inner, name),
                    }
                }
                t::IdlType::Defined { name, generics } => {
                    if generics.is_empty() {
                        Self::Defined(name)
                    } else {
                        Self::DefinedWithTypeArgs {
                            name,
                            args: generics
                                .into_iter()
                                .map(IdlDefinedTypeArg::try_from)
                                .collect::<Result<_>>()?,
                        }
                    }
                }
                t::IdlType::Generic(name) => Self::Generic(name),
                other => {
                    return Err(anyhow!(
                        "IDL type variant {other:?} has no legacy IDL equivalent"
                    ))
                }
            })
        }
    }

    impl TryFrom<t::IdlGenericArg> for IdlDefinedTypeArg {
        type Error = anyhow::Error;

        fn try_from(arg: t::IdlGenericArg) -> Result<Self> {
            Ok(match arg {
                t::IdlGenericArg::Type { ty } => match ty {
                    t::IdlType::Generic(name) => Self::Generic(name),
                    other => Self::Type(other.try_into()?),
                },
                t::IdlGenericArg::Const { value } => Self::Value(value),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip `external_legacy.json` through the current spec and
    /// back: legacy -> current -> legacy. The output JSON must equal the
    /// input. This exercises both converter directions on a real
    /// Anchor-emitted IDL.
    #[test]
    fn external_legacy_round_trip() {
        let original = include_bytes!("../../tests/declare-program/idls/external_legacy.json");
        let expected: serde_json::Value = serde_json::from_slice(original).unwrap();

        let current = convert_idl(original).expect("Converting legacy -> current failed");
        let legacy_bytes =
            convert_idl_to_legacy(&current).expect("Converting current -> legacy failed");
        let actual: serde_json::Value = serde_json::from_slice(&legacy_bytes).unwrap();

        assert_eq!(actual, expected);
    }
}
