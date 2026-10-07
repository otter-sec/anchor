//! Helpers for resolved IDL type references. Called only during IDL builds.
use {
    crate::{idl_build::__idl_json_string, IdlAccountType},
    alloc::{format, string::String, vec::Vec},
};

#[doc(hidden)]
#[derive(Clone)]
pub enum IdlGenericArg {
    Type(String),
    Const(String),
}
impl IdlGenericArg {
    fn json(&self) -> String {
        match self {
            Self::Type(ty) => format!("{{\"kind\":\"type\",\"type\":{ty}}}"),
            Self::Const(value) => format!(
                "{{\"kind\":\"const\",\"value\":{}}}",
                __idl_json_string(value)
            ),
        }
    }
}

#[doc(hidden)]
pub fn __idl_defined_reference(name: &str, args: &[IdlGenericArg]) -> String {
    let name = __idl_json_string(name);
    if args.is_empty() {
        format!("{{\"defined\":{{\"name\":{name}}}}}")
    } else {
        format!(
            "{{\"defined\":{{\"name\":{name},\"generics\":[{}]}}}}",
            args.iter()
                .map(IdlGenericArg::json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
#[doc(hidden)]
pub fn __idl_override_args(args: &mut [IdlGenericArg], overrides: &[Option<IdlGenericArg>]) {
    for (arg, value) in args.iter_mut().zip(overrides) {
        if let Some(value) = value {
            *arg = value.clone();
        }
    }
}
#[doc(hidden)]
pub fn __idl_type_argument<T: IdlAccountType + ?Sized>(
    args: &[Option<IdlGenericArg>],
    index: usize,
) -> String {
    match args.get(index) {
        Some(Some(IdlGenericArg::Type(ty))) => ty.clone(),
        _ => T::__idl_type_reference(&[]),
    }
}
#[doc(hidden)]
pub fn __idl_tuple_definition(name: &str, params: &[&str]) -> &'static str {
    let generics = params
        .iter()
        .map(|name| format!("{{\"kind\":\"type\",\"name\":{}}}", __idl_json_string(name)))
        .collect::<Vec<_>>()
        .join(",");
    let fields = params
        .iter()
        .map(|name| format!("{{\"generic\":{}}}", __idl_json_string(name)))
        .collect::<Vec<_>>()
        .join(",");
    let definition = format!(
        "{{\"name\":{},\"generics\":[{generics}],\"type\":{{\"kind\":\"struct\",\"fields\":\
         [{fields}]}}}}",
        __idl_json_string(name)
    );
    alloc::boxed::Box::leak(definition.into_boxed_str())
}
