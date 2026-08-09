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

/// Legacy IDL spec (pre Anchor v0.30)
mod legacy {
    use {
        crate::types as t,
        anyhow::{anyhow, Result},
        heck::SnakeCase,
        serde::{Deserialize, Serialize},
    };

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
        Struct {
            fields: Vec<IdlField>,
        },
        Enum {
            #[serde(default, rename = "tagEncoding")]
            tag_encoding: Option<String>,
            variants: Vec<IdlEnumVariant>,
        },
        Alias {
            value: IdlType,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct IdlEnumVariant {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub fields: Option<EnumFields>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub tag: Option<String>,
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
                instructions: idl.instructions.into_iter().map(Into::into).collect(),
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

    impl From<IdlInstruction> for t::IdlInstruction {
        fn from(value: IdlInstruction) -> Self {
            let name = value.name.to_snake_case();
            Self {
                discriminator: get_disc("global", &name),
                name,
                docs: value.docs.unwrap_or_default(),
                accounts: value.accounts.into_iter().map(Into::into).collect(),
                args: value.args.into_iter().map(Into::into).collect(),
                returns: value.returns.map(|r| r.into()),
            }
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
                IdlTypeDefinitionTy::Enum {
                    variants,
                    tag_encoding,
                } => Self::Enum {
                    tag_encoding,
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
                            tag: variant.tag,
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

    impl From<IdlAccountItem> for t::IdlInstructionAccountItem {
        fn from(value: IdlAccountItem) -> Self {
            match value {
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
                        .transpose()
                        .unwrap_or_default(),
                    relations: acc.relations,
                }),
                IdlAccountItem::IdlAccounts(accs) => Self::Composite(t::IdlInstructionAccounts {
                    name: accs.name.to_snake_case(),
                    accounts: accs.accounts.into_iter().map(Into::into).collect(),
                }),
            }
        }
    }

    impl TryFrom<IdlSeed> for t::IdlSeed {
        type Error = anyhow::Error;

        fn try_from(value: IdlSeed) -> Result<Self> {
            let seed = match value {
                IdlSeed::Account(seed) => Self::Account(t::IdlSeedAccount {
                    account: seed.account,
                    path: seed.path,
                }),
                IdlSeed::Arg(seed) => Self::Arg(t::IdlSeedArg { path: seed.path }),
                IdlSeed::Const(seed) => Self::Const(t::IdlSeedConst {
                    value: match seed.ty {
                        IdlType::String => seed.value.to_string().as_bytes().into(),
                        _ => return Err(anyhow!("Const seed conversion not supported")),
                    },
                }),
            };
            Ok(seed)
        }
    }
}
