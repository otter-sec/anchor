use {
    anyhow::anyhow,
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

/// IDL specification Semantic Version
pub const IDL_SPEC: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Idl {
    pub address: String,
    pub metadata: IdlMetadata,
    #[serde(default, skip_serializing_if = "is_default")]
    pub docs: Vec<String>,
    pub instructions: Vec<IdlInstruction>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub accounts: Vec<IdlAccount>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub events: Vec<IdlEvent>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub errors: Vec<IdlErrorCode>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub types: Vec<IdlTypeDef>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub constants: Vec<IdlConst>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    pub spec: String,
    #[serde(skip_serializing_if = "is_default")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub dependencies: Vec<IdlDependency>,
    #[serde(skip_serializing_if = "is_default")]
    pub contact: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    pub deployments: Option<IdlDeployments>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlDependency {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlDeployments {
    pub mainnet: Option<String>,
    pub testnet: Option<String>,
    pub devnet: Option<String>,
    pub localnet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlInstruction {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub docs: Vec<String>,
    pub discriminator: IdlDiscriminator,
    pub accounts: Vec<IdlInstructionAccountItem>,
    pub args: Vec<IdlField>,
    #[serde(skip_serializing_if = "is_default")]
    pub returns: Option<IdlType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum IdlInstructionAccountItem {
    Composite(IdlInstructionAccounts),
    Single(IdlInstructionAccount),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlInstructionAccount {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub writable: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub signer: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub optional: bool,
    #[serde(skip_serializing_if = "is_default")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    pub pda: Option<IdlPda>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlInstructionAccounts {
    pub name: String,
    pub accounts: Vec<IdlInstructionAccountItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
    #[serde(skip_serializing_if = "is_default")]
    pub program: Option<IdlSeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IdlSeed {
    Const(IdlSeedConst),
    Arg(IdlSeedArg),
    Account(IdlSeedAccount),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlSeedConst {
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlSeedArg {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlSeedAccount {
    pub path: String,
    #[serde(skip_serializing_if = "is_default")]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlAccount {
    pub name: String,
    pub discriminator: IdlDiscriminator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlEvent {
    pub name: String,
    pub discriminator: IdlDiscriminator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlConst {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: IdlType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdlErrorCode {
    pub code: u32,
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    pub msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlField {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub docs: Vec<String>,
    #[serde(rename = "type")]
    pub ty: IdlType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlTypeDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub docs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub serialization: IdlSerialization,
    #[serde(skip_serializing_if = "is_default")]
    pub repr: Option<IdlRepr>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub generics: Vec<IdlTypeDefGeneric>,
    #[serde(rename = "type")]
    pub ty: IdlTypeDefTy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum IdlSerialization {
    #[default]
    Borsh,
    Bytemuck,
    BytemuckUnsafe,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub enum IdlRepr {
    Rust(IdlReprModifier),
    C(IdlReprModifier),
    Transparent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlReprModifier {
    #[serde(default, skip_serializing_if = "is_default")]
    pub packed: bool,
    #[serde(skip_serializing_if = "is_default")]
    pub align: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IdlTypeDefGeneric {
    Type {
        name: String,
    },
    Const {
        name: String,
        #[serde(rename = "type")]
        ty: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IdlTypeDefTy {
    Struct {
        #[serde(skip_serializing_if = "is_default")]
        fields: Option<IdlDefinedFields>,
    },
    Enum {
        #[serde(default, rename = "tagEncoding", skip_serializing_if = "is_default")]
        tag_encoding: Option<String>,
        variants: Vec<IdlEnumVariant>,
    },
    Type {
        alias: IdlType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdlEnumVariant {
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    pub fields: Option<IdlDefinedFields>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum IdlDefinedFields {
    Named(Vec<IdlField>),
    Tuple(Vec<IdlType>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IdlArrayLen {
    Generic(String),
    #[serde(untagged)]
    Value(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IdlGenericArg {
    Type {
        #[serde(rename = "type")]
        ty: IdlType,
    },
    Const {
        value: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
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
    Pubkey,
    Option(Box<IdlType>),
    Vec(Box<IdlType>),
    Array(Box<IdlType>, IdlArrayLen),
    Defined {
        name: String,
        #[serde(default, skip_serializing_if = "is_default")]
        generics: Vec<IdlGenericArg>,
    },
    Generic(String),
}

// TODO: Move to utils crate
impl FromStr for IdlType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_owned();
        s.retain(|c| !c.is_whitespace());

        let r = match s.as_str() {
            "bool" => IdlType::Bool,
            "u8" => IdlType::U8,
            "i8" => IdlType::I8,
            "u16" => IdlType::U16,
            "i16" => IdlType::I16,
            "u32" => IdlType::U32,
            "i32" => IdlType::I32,
            "f32" => IdlType::F32,
            "u64" => IdlType::U64,
            "i64" => IdlType::I64,
            "f64" => IdlType::F64,
            "u128" => IdlType::U128,
            "i128" => IdlType::I128,
            "u256" => IdlType::U256,
            "i256" => IdlType::I256,
            "Vec<u8>" => IdlType::Bytes,
            "String" | "&str" | "&'staticstr" => IdlType::String,
            "Pubkey" => IdlType::Pubkey,
            _ => {
                if let Some(inner) = s.strip_prefix("Option<") {
                    let inner_ty = Self::from_str(
                        inner
                            .strip_suffix('>')
                            .ok_or_else(|| anyhow!("Invalid Option"))?,
                    )?;
                    return Ok(IdlType::Option(Box::new(inner_ty)));
                }

                if let Some(inner) = s.strip_prefix("Vec<") {
                    let inner_ty = Self::from_str(
                        inner
                            .strip_suffix('>')
                            .ok_or_else(|| anyhow!("Invalid Vec"))?,
                    )?;
                    return Ok(IdlType::Vec(Box::new(inner_ty)));
                }

                if s.starts_with('[') {
                    fn array_from_str(inner: &str) -> IdlType {
                        match inner.strip_suffix(']') {
                            Some(nested_inner) => array_from_str(&nested_inner[1..]),
                            None => {
                                let (raw_type, raw_length) = inner.rsplit_once(';').unwrap();
                                let ty = IdlType::from_str(raw_type).unwrap();
                                let len = match raw_length.replace('_', "").parse::<usize>() {
                                    Ok(len) => IdlArrayLen::Value(len),
                                    Err(_) => IdlArrayLen::Generic(raw_length.to_owned()),
                                };
                                IdlType::Array(Box::new(ty), len)
                            }
                        }
                    }
                    return Ok(array_from_str(&s));
                }

                // Defined
                let (name, generics) = if let Some(i) = s.find('<') {
                    (
                        s.get(..i).unwrap().to_owned(),
                        s.get(i + 1..)
                            .unwrap()
                            .strip_suffix('>')
                            .unwrap()
                            .split(',')
                            .map(|g| g.trim().to_owned())
                            .map(|g| {
                                if g.parse::<bool>().is_ok()
                                    || g.parse::<u128>().is_ok()
                                    || g.parse::<i128>().is_ok()
                                    || g.parse::<char>().is_ok()
                                {
                                    Ok(IdlGenericArg::Const { value: g })
                                } else {
                                    Self::from_str(&g).map(|ty| IdlGenericArg::Type { ty })
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                } else {
                    (s.to_owned(), vec![])
                };

                IdlType::Defined { name, generics }
            }
        };
        Ok(r)
    }
}

pub type IdlDiscriminator = Vec<u8>;

/// Get whether the given data is the default of its type.
fn is_default<T: Default + PartialEq>(it: &T) -> bool {
    *it == T::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option() {
        assert_eq!(
            IdlType::from_str("Option<bool>").unwrap(),
            IdlType::Option(Box::new(IdlType::Bool))
        )
    }

    #[test]
    fn vector() {
        assert_eq!(
            IdlType::from_str("Vec<bool>").unwrap(),
            IdlType::Vec(Box::new(IdlType::Bool))
        )
    }

    #[test]
    fn array() {
        assert_eq!(
            IdlType::from_str("[Pubkey; 16]").unwrap(),
            IdlType::Array(Box::new(IdlType::Pubkey), IdlArrayLen::Value(16))
        );
    }

    #[test]
    fn array_with_underscored_length() {
        assert_eq!(
            IdlType::from_str("[u8; 50_000]").unwrap(),
            IdlType::Array(Box::new(IdlType::U8), IdlArrayLen::Value(50000))
        );
    }

    #[test]
    fn multidimensional_array() {
        assert_eq!(
            IdlType::from_str("[[u8; 16]; 32]").unwrap(),
            IdlType::Array(
                Box::new(IdlType::Array(
                    Box::new(IdlType::U8),
                    IdlArrayLen::Value(16)
                )),
                IdlArrayLen::Value(32)
            )
        );
    }

    #[test]
    fn generic_array() {
        assert_eq!(
            IdlType::from_str("[u64; T]").unwrap(),
            IdlType::Array(Box::new(IdlType::U64), IdlArrayLen::Generic("T".into()))
        );
    }

    #[test]
    fn defined() {
        assert_eq!(
            IdlType::from_str("MyStruct").unwrap(),
            IdlType::Defined {
                name: "MyStruct".into(),
                generics: vec![]
            }
        )
    }

    #[test]
    fn defined_with_generics() {
        assert_eq!(
            IdlType::from_str("MyStruct<Pubkey, u64, 8>").unwrap(),
            IdlType::Defined {
                name: "MyStruct".into(),
                generics: vec![
                    IdlGenericArg::Type {
                        ty: IdlType::Pubkey
                    },
                    IdlGenericArg::Type { ty: IdlType::U64 },
                    IdlGenericArg::Const { value: "8".into() },
                ],
            }
        )
    }
}
