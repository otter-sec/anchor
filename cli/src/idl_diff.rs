//! `anchor idl diff`: compare the account layouts of two IDLs and report changes that would make
//! the new program misread accounts written by the old one.

use {
    anchor_lang_idl::types::{
        Idl, IdlArrayLen, IdlDefinedFields, IdlEnumVariant, IdlField, IdlGenericArg,
        IdlSerialization, IdlType, IdlTypeDef, IdlTypeDefGeneric, IdlTypeDefTy,
    },
    std::{
        collections::{HashMap, HashSet},
        fmt,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Same bytes; nothing to do.
    Info,
    /// Existing accounts are too short for the new type and must be reallocated.
    Migration,
    /// Existing accounts would be rejected or decoded into wrong values.
    Break,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Level::Info => "info",
            Level::Migration => "needs migration",
            Level::Break => "breaking",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub account: String,
    pub message: String,
}

/// Compares every account type of `old` with the same account in `new`.
pub fn diff(old: &Idl, new: &Idl) -> Vec<Finding> {
    let mut out = vec![];
    for acc in &old.accounts {
        let mut push = |level, message: String| {
            out.push(Finding {
                level,
                account: acc.name.clone(),
                message,
            })
        };
        let new_acc = match new.accounts.iter().find(|a| a.name == acc.name) {
            Some(a) => a,
            None => match new
                .accounts
                .iter()
                .find(|a| a.discriminator == acc.discriminator)
            {
                Some(a) => {
                    push(
                        Level::Info,
                        format!("renamed to `{}` with the same discriminator", a.name),
                    );
                    a
                }
                None => {
                    push(
                        Level::Break,
                        "removed or renamed: no account in the new IDL has its discriminator, so \
                         existing accounts fail with AccountDiscriminatorMismatch (3002). Keep \
                         the name, or set `#[account(discriminator = ...)]` to the old value"
                            .into(),
                    );
                    continue;
                }
            },
        };
        if new_acc.discriminator != acc.discriminator {
            push(
                Level::Break,
                format!(
                    "discriminator changed from {:?} to {:?}: existing accounts fail with \
                     AccountDiscriminatorMismatch (3002)",
                    acc.discriminator, new_acc.discriminator
                ),
            );
            continue;
        }
        let (old_def, new_def) = match (find(old, &acc.name), find(new, &new_acc.name)) {
            (Some(a), Some(b)) => (a, b),
            (a, _) => {
                push(
                    Level::Break,
                    undefined(&acc.name, &new_acc.name, a.is_none()),
                );
                continue;
            }
        };
        if let Some(message) = encoding_change(old_def, new_def) {
            push(Level::Break, message);
            continue;
        }
        let mut cx = Cx {
            old,
            new,
            findings: vec![],
            // Not seeded with the account: a nested copy of it (`Vec<Self>`)
            // must be compared as a nested value, where appended fields break.
            visiting: HashSet::new(),
            depth: 0,
        };
        cx.compare_def(&old_def.ty, &new_def.ty, &acc.name, true);
        for (level, message) in cx.findings {
            push(level, message);
        }
    }
    out
}

const WRONG_OFFSET: &str = "this field and the ones after it are read at the wrong offset, so \
                            existing accounts decode into wrong values";

fn find<'a>(idl: &'a Idl, name: &str) -> Option<&'a IdlTypeDef> {
    idl.types.iter().find(|t| t.name == name)
}

/// A type the IDL references but does not define (for example from a crate built without
/// `idl-build`) cannot be checked; saying nothing would read as "unchanged".
fn undefined(old_name: &str, new_name: &str, missing_in_old: bool) -> String {
    let (name, side) = if missing_in_old {
        (old_name, "old")
    } else {
        (new_name, "new")
    };
    format!("not checked: type `{name}` is not defined in the {side} IDL, so its layout is unknown")
}

/// Serialization always matters. `repr` only changes the bytes of zero-copy types: Borsh
/// ignores it.
fn encoding_change(a: &IdlTypeDef, b: &IdlTypeDef) -> Option<String> {
    if a.serialization != b.serialization {
        return Some(
            "serialization changed: existing bytes are read with a different encoding".into(),
        );
    }
    if a.serialization != IdlSerialization::Borsh && a.repr != b.repr {
        return Some("repr changed: field offsets of existing zero-copy bytes change".into());
    }
    None
}

/// The definition of a generic type with its parameters replaced by the arguments it is used
/// with, so `Wrapper<Inner>` is compared through `Inner`'s layout.
fn instantiate(def: &IdlTypeDef, args: &[IdlGenericArg]) -> Option<IdlTypeDefTy> {
    if def.generics.len() != args.len() {
        return None;
    }
    let env: HashMap<&str, &IdlGenericArg> = def
        .generics
        .iter()
        .map(|g| match g {
            IdlTypeDefGeneric::Type { name } | IdlTypeDefGeneric::Const { name, .. } => {
                name.as_str()
            }
        })
        .zip(args)
        .collect();
    let fields = |f: &Option<IdlDefinedFields>| {
        f.as_ref().map(|f| match f {
            IdlDefinedFields::Named(fs) => IdlDefinedFields::Named(
                fs.iter()
                    .map(|f| IdlField {
                        ty: subst(&f.ty, &env),
                        ..f.clone()
                    })
                    .collect(),
            ),
            IdlDefinedFields::Tuple(ts) => {
                IdlDefinedFields::Tuple(ts.iter().map(|t| subst(t, &env)).collect())
            }
        })
    };
    Some(match &def.ty {
        IdlTypeDefTy::Struct { fields: f } => IdlTypeDefTy::Struct { fields: fields(f) },
        IdlTypeDefTy::Enum { variants } => IdlTypeDefTy::Enum {
            variants: variants
                .iter()
                .map(|v| IdlEnumVariant {
                    name: v.name.clone(),
                    fields: fields(&v.fields),
                })
                .collect(),
        },
        IdlTypeDefTy::Type { alias } => IdlTypeDefTy::Type {
            alias: subst(alias, &env),
        },
    })
}

fn subst(t: &IdlType, env: &HashMap<&str, &IdlGenericArg>) -> IdlType {
    match t {
        IdlType::Generic(g) => match env.get(g.as_str()) {
            Some(IdlGenericArg::Type { ty }) => ty.clone(),
            _ => t.clone(),
        },
        IdlType::Option(x) => IdlType::Option(Box::new(subst(x, env))),
        IdlType::Vec(x) => IdlType::Vec(Box::new(subst(x, env))),
        IdlType::Array(x, l) => {
            let l = match l {
                IdlArrayLen::Generic(g) => match env.get(g.as_str()) {
                    Some(IdlGenericArg::Const { value }) => value
                        .parse()
                        .map(IdlArrayLen::Value)
                        .unwrap_or_else(|_| l.clone()),
                    _ => l.clone(),
                },
                _ => l.clone(),
            };
            IdlType::Array(Box::new(subst(x, env)), l)
        }
        IdlType::Defined { name, generics } => IdlType::Defined {
            name: name.clone(),
            generics: generics
                .iter()
                .map(|g| match g {
                    // The IDL generator emits a const parameter passed on as `Inner<N>` as a
                    // type argument when its name is one letter.
                    IdlGenericArg::Type {
                        ty: IdlType::Generic(n),
                    } if matches!(env.get(n.as_str()), Some(IdlGenericArg::Const { .. })) => {
                        (*env[n.as_str()]).clone()
                    }
                    IdlGenericArg::Type { ty } => IdlGenericArg::Type { ty: subst(ty, env) },
                    other => other.clone(),
                })
                .collect(),
        },
        _ => t.clone(),
    }
}

struct Cx<'a> {
    old: &'a Idl,
    new: &'a Idl,
    findings: Vec<(Level, String)>,
    /// Pairs of defined types being compared, to stop on recursive types
    /// such as `Vec<Self>` or `Option<Box<Self>>`.
    visiting: HashSet<(String, String)>,
    /// Nesting depth of defined types, to stop on recursion whose generic arguments grow at
    /// each level (`Node<T> { next: Vec<Node<Option<T>>> }`), which `visiting` cannot catch.
    depth: usize,
}

const MAX_DEPTH: usize = 64;

impl Cx<'_> {
    fn add(&mut self, level: Level, message: String) {
        self.findings.push((level, message));
    }

    /// Returns false when the bytes of `path` and everything after it are read differently.
    fn compare_def(&mut self, a: &IdlTypeDefTy, b: &IdlTypeDefTy, path: &str, top: bool) -> bool {
        match (a, b) {
            (IdlTypeDefTy::Struct { fields: fa }, IdlTypeDefTy::Struct { fields: fb }) => {
                self.compare_fields(fa.as_ref(), fb.as_ref(), path, top)
            }
            (IdlTypeDefTy::Enum { variants: va }, IdlTypeDefTy::Enum { variants: vb }) => {
                for (i, (x, y)) in va.iter().zip(vb).enumerate() {
                    if x.name != y.name {
                        self.add(
                            Level::Break,
                            format!(
                                "{path}: variant {i} changed from `{}` to `{}`; Borsh stores the \
                                 variant index, so stored values change meaning",
                                x.name, y.name
                            ),
                        );
                        return false;
                    }
                    let p = format!("{path}::{}", x.name);
                    if !self.compare_fields(x.fields.as_ref(), y.fields.as_ref(), &p, false) {
                        return false;
                    }
                }
                if vb.len() < va.len() {
                    let gone: Vec<_> = va[vb.len()..].iter().map(|v| v.name.as_str()).collect();
                    self.add(
                        Level::Break,
                        format!("{path}: variants removed: {}", gone.join(", ")),
                    );
                    return false;
                }
                let largest = |vs: &[IdlEnumVariant], idl: &Idl| {
                    vs.iter()
                        .map(|v| variant_size(v, idl))
                        .collect::<Option<Vec<_>>>()
                        .and_then(|s| s.into_iter().max())
                };
                if let (Some(old_max), Some(new_max)) =
                    (largest(va, self.old), largest(vb, self.new))
                {
                    if new_max > old_max {
                        let added: Vec<_> =
                            vb[va.len()..].iter().map(|v| v.name.as_str()).collect();
                        self.add(
                            Level::Migration,
                            format!(
                                "{path}: variants added: {}; the largest variant is now {new_max} \
                                 bytes (was {old_max}), so accounts sized for the old largest \
                                 fail with AccountDidNotSerialize (3004) when storing one until \
                                 they are reallocated",
                                added.join(", ")
                            ),
                        );
                    }
                }
                true
            }
            (IdlTypeDefTy::Type { alias: x }, IdlTypeDefTy::Type { alias: y }) => {
                self.compare_ty(x, y, path)
            }
            _ => {
                self.add(Level::Break, format!("{path}: kind of type changed"));
                false
            }
        }
    }

    fn compare_fields(
        &mut self,
        a: Option<&IdlDefinedFields>,
        b: Option<&IdlDefinedFields>,
        path: &str,
        top: bool,
    ) -> bool {
        let named = |f: Option<&IdlDefinedFields>| -> Vec<(String, IdlType)> {
            match f {
                None => vec![],
                Some(IdlDefinedFields::Named(fs)) => {
                    fs.iter().map(|f| (f.name.clone(), f.ty.clone())).collect()
                }
                Some(IdlDefinedFields::Tuple(ts)) => ts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (i.to_string(), t.clone()))
                    .collect(),
            }
        };
        let (fa, fb) = (named(a), named(b));
        for (i, ((na, ta), (nb, tb))) in fa.iter().zip(&fb).enumerate() {
            let p = format!("{path}.{nb}");
            let before = self.findings.len();
            if !self.compare_ty(ta, tb, &p) {
                // Report one line per changed field: the inner detail plus its consequence.
                let inner: Vec<_> = self.findings.split_off(before);
                let detail = if na != nb {
                    let moved = fa
                        .iter()
                        .position(|(n, _)| n == nb)
                        .filter(|&j| j != i)
                        .map(|j| format!(" (`{nb}` moved from position {j})"))
                        .unwrap_or_default();
                    format!(
                        "{path}: field {i} was `{na}: {}` and is now `{nb}: {}`{moved}",
                        name(ta),
                        name(tb)
                    )
                } else {
                    // The breaking one, not an earlier rename or migration note.
                    inner
                        .into_iter()
                        .find(|(l, _)| *l == Level::Break)
                        .map(|(_, m)| m)
                        .unwrap_or_else(|| format!("{p}: changed"))
                };
                // A nested field already explains the consequence; add it only once.
                let message = if detail.contains("change meaning") || detail.contains(WRONG_OFFSET)
                {
                    detail
                } else {
                    format!("{detail}; {WRONG_OFFSET}")
                };
                self.add(Level::Break, message);
                return false;
            }
            if na != nb {
                // Same type at this position, but if either name exists at another position
                // the field moved (swap, insert or removal mid-struct), so stored bytes now
                // belong to a different field.
                let moved: Vec<_> = fa
                    .iter()
                    .enumerate()
                    .filter_map(|(j, (n, _))| {
                        let k = fb.iter().position(|(m, _)| m == n)?;
                        (j != k).then(|| format!("`{n}` (position {j} -> {k})"))
                    })
                    .collect();
                if !moved.is_empty() {
                    self.add(
                        Level::Break,
                        format!(
                            "{path}: fields moved: {}; existing accounts keep the old order, so \
                             these fields decode into wrong values",
                            moved.join(", ")
                        ),
                    );
                    return false;
                }
                self.add(
                    Level::Info,
                    format!(
                        "{p}: renamed from `{na}` (same bytes; clients using the old name break)"
                    ),
                );
            }
        }
        if fb.len() < fa.len() {
            let gone: Vec<_> = fa[fb.len()..].iter().map(|(n, _)| n.as_str()).collect();
            if top {
                self.add(
                    Level::Info,
                    format!(
                        "{path}: fields removed at the end: {} (old bytes stay; the next write \
                         drops them)",
                        gone.join(", ")
                    ),
                );
                return true;
            }
            self.add(
                Level::Break,
                format!("{path}: fields removed: {}", gone.join(", ")),
            );
            return false;
        }
        if fb.len() > fa.len() {
            let added = &fb[fa.len()..];
            let names: Vec<_> = added.iter().map(|(n, _)| n.as_str()).collect();
            if !top {
                self.add(
                    Level::Break,
                    format!(
                        "{path}: fields added: {}; everything after this value shifts",
                        names.join(", ")
                    ),
                );
                return false;
            }
            let size = added
                .iter()
                .map(|(_, t)| fixed_size(t, self.new))
                .sum::<Option<usize>>()
                .map(|n| format!("{n} more byte{}", if n == 1 { "" } else { "s" }))
                .unwrap_or_else(|| "more bytes".into());
            self.add(
                Level::Migration,
                format!(
                    "{path}: fields appended: {} ({size}). Accounts created by the old program \
                     are too short and fail with AccountDidNotDeserialize (3003) until each is \
                     reallocated (`realloc` or `Migration<From, To>`). Accounts created with \
                     spare space decode instead, and the new fields read whatever bytes are there \
                     (zeros, or leftovers if an earlier variable-length field shrank), so set \
                     them explicitly",
                    names.join(", ")
                ),
            );
        }
        true
    }

    fn compare_ty(&mut self, a: &IdlType, b: &IdlType, path: &str) -> bool {
        match (a, b) {
            (IdlType::Option(x), IdlType::Option(y)) | (IdlType::Vec(x), IdlType::Vec(y)) => {
                self.compare_ty(x, y, path)
            }
            (IdlType::Array(x, lx), IdlType::Array(y, ly)) => {
                if lx != ly {
                    self.add(
                        Level::Break,
                        format!(
                            "{path}: array length changed from {} to {}",
                            len(lx),
                            len(ly)
                        ),
                    );
                    return false;
                }
                self.compare_ty(x, y, path)
            }
            (
                IdlType::Defined {
                    name: na,
                    generics: ga,
                },
                IdlType::Defined {
                    name: nb,
                    generics: gb,
                },
            ) => {
                let (da, db) = match (find(self.old, na), find(self.new, nb)) {
                    (Some(da), Some(db)) => (da, db),
                    (da, _) => {
                        self.add(
                            Level::Break,
                            format!("{path}: {}", undefined(na, nb, da.is_none())),
                        );
                        return false;
                    }
                };
                if let Some(message) = encoding_change(da, db) {
                    self.add(Level::Break, format!("{path}: {message}"));
                    return false;
                }
                let (ta, tb) = match (instantiate(da, ga), instantiate(db, gb)) {
                    (Some(ta), Some(tb)) => (ta, tb),
                    (ta, _) => {
                        let n = if ta.is_none() { na } else { nb };
                        self.add(
                            Level::Break,
                            format!(
                                "{path}: not checked: generic arguments of `{n}` do not match its \
                                 parameters"
                            ),
                        );
                        return false;
                    }
                };
                // Already compared further up: its changes are reported there.
                let pair = (format!("{na}{ga:?}"), format!("{nb}{gb:?}"));
                if !self.visiting.insert(pair.clone()) {
                    return true;
                }
                if self.depth == MAX_DEPTH {
                    self.add(
                        Level::Break,
                        format!(
                            "{path}: not checked: types nest more than {MAX_DEPTH} levels deep"
                        ),
                    );
                    return false;
                }
                self.depth += 1;
                let same = self.compare_def(&ta, &tb, path, false);
                self.depth -= 1;
                self.visiting.remove(&pair);
                same
            }
            _ if a == b => true,
            _ => {
                self.add(
                    Level::Break,
                    format!("{path}: type changed from {} to {}", name(a), name(b)),
                );
                false
            }
        }
    }
}

fn len(l: &IdlArrayLen) -> String {
    match l {
        IdlArrayLen::Value(n) => n.to_string(),
        IdlArrayLen::Generic(g) => g.clone(),
    }
}

fn name(t: &IdlType) -> String {
    match t {
        IdlType::Defined { name, .. } => name.clone(),
        IdlType::Option(x) => format!("Option<{}>", name(x)),
        IdlType::Vec(x) => format!("Vec<{}>", name(x)),
        IdlType::Array(x, l) => format!("[{}; {}]", name(x), len(l)),
        other => serde_json::to_value(other)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{other:?}")),
    }
}

/// Borsh size of an enum variant: the 1-byte index plus its fields.
fn variant_size(v: &IdlEnumVariant, idl: &Idl) -> Option<usize> {
    let fields = match &v.fields {
        None => 0,
        Some(IdlDefinedFields::Named(fs)) => fs
            .iter()
            .map(|f| fixed_size(&f.ty, idl))
            .sum::<Option<usize>>()?,
        Some(IdlDefinedFields::Tuple(ts)) => ts
            .iter()
            .map(|t| fixed_size(t, idl))
            .sum::<Option<usize>>()?,
    };
    Some(1 + fields)
}

/// Borsh size of a value of this type, or None when it varies.
fn fixed_size(t: &IdlType, idl: &Idl) -> Option<usize> {
    Some(match t {
        IdlType::Bool | IdlType::U8 | IdlType::I8 => 1,
        IdlType::U16 | IdlType::I16 => 2,
        IdlType::U32 | IdlType::I32 | IdlType::F32 => 4,
        IdlType::U64 | IdlType::I64 | IdlType::F64 => 8,
        IdlType::U128 | IdlType::I128 => 16,
        IdlType::U256 | IdlType::I256 | IdlType::Pubkey => 32,
        IdlType::Array(x, IdlArrayLen::Value(n)) => fixed_size(x, idl)? * n,
        IdlType::Defined { name, generics } if generics.is_empty() => match &find(idl, name)?.ty {
            IdlTypeDefTy::Struct { fields } => match fields {
                None => 0,
                Some(IdlDefinedFields::Named(fs)) => fs
                    .iter()
                    .map(|f: &IdlField| fixed_size(&f.ty, idl))
                    .sum::<Option<usize>>()?,
                Some(IdlDefinedFields::Tuple(ts)) => ts
                    .iter()
                    .map(|t| fixed_size(t, idl))
                    .sum::<Option<usize>>()?,
            },
            IdlTypeDefTy::Enum { variants } if variants.iter().all(|v| v.fields.is_none()) => 1,
            IdlTypeDefTy::Type { alias } => fixed_size(alias, idl)?,
            _ => return None,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn idl(name: &str, disc: u8, fields: serde_json::Value, extra: serde_json::Value) -> Idl {
        let mut types = vec![json!({"name": name, "type": {"kind": "struct", "fields": fields}})];
        if let Some(e) = extra.as_array() {
            types.extend(e.iter().cloned());
        }
        serde_json::from_value(json!({
            "address": "11111111111111111111111111111111",
            "metadata": {"name": "p", "version": "0.1.0", "spec": "0.1.0"},
            "instructions": [],
            "accounts": [{"name": name, "discriminator": [disc, 0, 0, 0, 0, 0, 0, 0]}],
            "types": types
        }))
        .unwrap()
    }

    fn counter(fields: serde_json::Value) -> Idl {
        idl("Counter", 1, fields, json!(null))
    }

    fn levels(old: &Idl, new: &Idl) -> Vec<Level> {
        diff(old, new).iter().map(|f| f.level).collect()
    }

    fn base() -> Idl {
        counter(json!([{"name": "count", "type": "u64"}, {"name": "authority", "type": "pubkey"}]))
    }

    #[test]
    fn unchanged_is_empty() {
        assert!(diff(&base(), &base()).is_empty());
    }

    #[test]
    fn append_needs_migration_with_size() {
        let new = counter(json!([
            {"name": "count", "type": "u64"}, {"name": "authority", "type": "pubkey"},
            {"name": "bump", "type": "u8"}
        ]));
        let f = diff(&base(), &new);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Migration);
        assert!(
            f[0].message.contains("bump (1 more byte)"),
            "{}",
            f[0].message
        );
    }

    #[test]
    fn reorder_and_type_change_break() {
        let swapped = counter(json!([
            {"name": "authority", "type": "pubkey"}, {"name": "count", "type": "u64"}
        ]));
        let f = diff(&base(), &swapped);
        assert!(f.iter().any(|f| f.level == Level::Break
            && f.message
                .contains("field 0 was `count: u64` and is now `authority: pubkey`")
            && f.message.contains("moved from position 1")));
        assert_eq!(f.len(), 1);
        let narrowed = counter(json!([
            {"name": "count", "type": "u32"}, {"name": "authority", "type": "pubkey"}
        ]));
        assert!(levels(&base(), &narrowed).contains(&Level::Break));
    }

    /// An IDL with one account `name` and the given type definitions (the account's included).
    fn with_types(name: &str, types: serde_json::Value) -> Idl {
        serde_json::from_value(json!({
            "address": "11111111111111111111111111111111",
            "metadata": {"name": "p", "version": "0.1.0", "spec": "0.1.0"},
            "instructions": [],
            "accounts": [{"name": name, "discriminator": [1, 0, 0, 0, 0, 0, 0, 0]}],
            "types": types
        }))
        .unwrap()
    }

    #[test]
    fn generic_arguments_are_compared_by_layout() {
        let vault = |inner_name: &str, inner_ty: &str| {
            with_types(
                "Vault",
                json!([
                    {"name": "Vault", "type": {"kind": "struct", "fields": [
                        {"name": "w", "type": {"defined": {"name": "Wrapper", "generics": [
                            {"kind": "type", "type": {"defined": {"name": inner_name}}}
                        ]}}},
                        {"name": "n", "type": "u64"}
                    ]}},
                    {"name": "Wrapper", "generics": [{"kind": "type", "name": "T"}],
                     "type": {"kind": "struct", "fields": [{"name": "value", "type": {"generic": "T"}}]}},
                    {"name": inner_name, "type": {"kind": "struct", "fields": [{"name": "a", "type": inner_ty}]}}
                ]),
            )
        };
        // The argument's layout changed: everything after it shifts.
        let f = diff(&vault("Inner", "u64"), &vault("Inner", "u32"));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Break);
        // Explained once, not once per nesting level.
        assert_eq!(
            f[0].message.matches(WRONG_OFFSET).count(),
            1,
            "{}",
            f[0].message
        );
        // The argument was renamed but has the same layout: same bytes.
        assert!(diff(&vault("Inner", "u64"), &vault("Inner2", "u64")).is_empty());
    }

    #[test]
    fn const_generic_passed_on_is_compared() {
        // `Outer<const N: usize> { inner: Inner<N> }`, `Inner<const M: usize> { data: [u8; M] }`.
        // The generator emits the one-letter `N` in `Inner<N>` as a type argument.
        let acc = |n: &str| {
            with_types(
                "Acc",
                json!([
                    {"name": "Acc", "type": {"kind": "struct", "fields": [
                        {"name": "o", "type": {"defined": {"name": "Outer", "generics": [{"kind": "const", "value": n}]}}}
                    ]}},
                    {"name": "Outer", "generics": [{"kind": "const", "name": "N", "type": "usize"}],
                     "type": {"kind": "struct", "fields": [
                        {"name": "inner", "type": {"defined": {"name": "Inner", "generics": [{"kind": "type", "type": {"generic": "N"}}]}}}
                    ]}},
                    {"name": "Inner", "generics": [{"kind": "const", "name": "M", "type": "usize"}],
                     "type": {"kind": "struct", "fields": [{"name": "data", "type": {"array": ["u8", {"generic": "M"}]}}]}}
                ]),
            )
        };
        let f = diff(&acc("32"), &acc("64"));
        assert_eq!(f.len(), 1);
        assert!(
            f[0].message.contains("array length changed from 32 to 64"),
            "{}",
            f[0].message
        );
        assert!(diff(&acc("32"), &acc("32")).is_empty());
    }

    #[test]
    fn growing_generic_recursion_terminates() {
        // `Node<T> { v: T, next: Vec<Node<Option<T>>> }`: the arguments differ at every level.
        let node = with_types(
            "Acc",
            json!([
                {"name": "Acc", "type": {"kind": "struct", "fields": [
                    {"name": "n", "type": {"defined": {"name": "Node", "generics": [{"kind": "type", "type": "u8"}]}}}
                ]}},
                {"name": "Node", "generics": [{"kind": "type", "name": "T"}],
                 "type": {"kind": "struct", "fields": [
                    {"name": "v", "type": {"generic": "T"}},
                    {"name": "next", "type": {"vec": {"defined": {"name": "Node", "generics": [
                        {"kind": "type", "type": {"option": {"generic": "T"}}}
                    ]}}}}
                ]}}
            ]),
        );
        let f = diff(&node, &node);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("not checked: types nest more than"));
    }

    #[test]
    fn nested_break_reports_its_cause() {
        let counter = |cfg: serde_json::Value| {
            idl(
                "Counter",
                1,
                json!([{"name": "cfg", "type": {"defined": {"name": "Cfg"}}}, {"name": "n", "type": "u64"}]),
                json!([{"name": "Cfg", "type": {"kind": "struct", "fields": cfg}}]),
            )
        };
        let old = counter(json!([{"name": "a", "type": "u64"}, {"name": "b", "type": "u32"}]));
        let new = counter(json!([{"name": "a2", "type": "u64"}, {"name": "b", "type": "u64"}]));
        let f = diff(&old, &new);
        assert_eq!(f.len(), 1);
        assert!(
            f[0].message.contains("type changed from u32 to u64"),
            "{}",
            f[0].message
        );
    }

    #[test]
    fn undefined_types_are_reported() {
        let cfg = json!({"name": "Config", "type": {"kind": "struct", "fields": [{"name": "a", "type": "u64"}]}});
        let counter = json!({"name": "Counter", "type": {"kind": "struct", "fields": [
            {"name": "cfg", "type": {"defined": {"name": "Config"}}}
        ]}});
        let defined = with_types("Counter", json!([counter, cfg]));
        let undefined = with_types("Counter", json!([counter]));
        let f = diff(&defined, &undefined);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Break);
        assert!(f[0]
            .message
            .contains("`Config` is not defined in the new IDL"));
        // The account's own definition missing is reported too, not skipped.
        let f = diff(&defined, &with_types("Counter", json!([cfg])));
        assert!(f[0]
            .message
            .contains("`Counter` is not defined in the new IDL"));
    }

    #[test]
    fn repr_matters_only_for_zero_copy() {
        let pool = |ser: &str, repr: serde_json::Value| {
            with_types(
                "Pool",
                json!([
                    {"name": "Pool", "serialization": ser, "repr": {"kind": "c"},
                     "type": {"kind": "struct", "fields": [{"name": "cfg", "type": {"defined": {"name": "Cfg"}}}]}},
                    {"name": "Cfg", "serialization": ser, "repr": repr,
                     "type": {"kind": "struct", "fields": [{"name": "a", "type": "u8"}, {"name": "b", "type": "u64"}]}}
                ]),
            )
        };
        let c = json!({"kind": "c"});
        let packed = json!({"kind": "c", "packed": true});
        // A nested zero-copy type became packed: its field offsets move.
        let f = diff(
            &pool("bytemuck", c.clone()),
            &pool("bytemuck", packed.clone()),
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Break);
        assert!(f[0].message.contains("Pool.cfg: repr changed"));
        // Borsh ignores repr.
        assert!(diff(&pool("borsh", c), &pool("borsh", packed)).is_empty());
    }

    #[test]
    fn larger_enum_variant_needs_migration() {
        let with_mode = |variants: serde_json::Value| {
            idl(
                "Counter",
                1,
                json!([{"name": "mode", "type": {"defined": {"name": "Mode"}}}]),
                json!([{"name": "Mode", "type": {"kind": "enum", "variants": variants}}]),
            )
        };
        let old = with_mode(json!([{"name": "A"}, {"name": "B", "fields": ["u8"]}]));
        let bigger = with_mode(json!([
            {"name": "A"}, {"name": "B", "fields": ["u8"]}, {"name": "C", "fields": [{"array": ["u8", 64]}]}
        ]));
        let f = diff(&old, &bigger);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Migration);
        assert!(f[0].message.contains("now 65 bytes (was 2)"));
        // A new variant no larger than the old largest fits in existing accounts.
        let same = with_mode(json!([
            {"name": "A"}, {"name": "B", "fields": ["u8"]}, {"name": "C", "fields": ["bool"]}
        ]));
        assert!(diff(&old, &same).is_empty());
    }

    #[test]
    fn same_type_moves_break() {
        let two = |a: &str, b: &str| {
            counter(json!([{"name": a, "type": "u64"}, {"name": b, "type": "u64"}]))
        };
        // Swap of two fields with the same type.
        let f = diff(&two("a", "b"), &two("b", "a"));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Break);
        assert!(f[0].message.contains("`a` (position 0 -> 1)"));
        assert!(f[0].message.contains("`b` (position 1 -> 0)"));

        // Insert in the middle: not an append, so realloc does not fix it.
        let inserted = counter(json!([
            {"name": "a", "type": "u64"}, {"name": "b", "type": "u64"},
            {"name": "c", "type": "u64"}
        ]));
        let f = diff(&two("a", "c"), &inserted);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, Level::Break);
        assert!(f[0].message.contains("`c` (position 1 -> 2)"));
        assert!(!f[0].message.contains("realloc"));

        // Removal in the middle: not a removal at the end.
        assert_eq!(levels(&inserted, &two("a", "c")), vec![Level::Break]);
    }

    #[test]
    fn rename_field_is_info_and_rename_account_breaks() {
        let renamed = counter(json!([
            {"name": "value", "type": "u64"}, {"name": "authority", "type": "pubkey"}
        ]));
        assert_eq!(levels(&base(), &renamed), vec![Level::Info]);
        let other = idl(
            "Tally",
            2,
            json!([{"name": "count", "type": "u64"}, {"name": "authority", "type": "pubkey"}]),
            json!(null),
        );
        assert_eq!(levels(&base(), &other), vec![Level::Break]);
    }

    #[test]
    fn enum_variants() {
        let with_mode = |variants: serde_json::Value| {
            idl(
                "Counter",
                1,
                json!([{"name": "mode", "type": {"defined": {"name": "Mode"}}},
                       {"name": "count", "type": "u64"}]),
                json!([{"name": "Mode", "type": {"kind": "enum", "variants": variants}}]),
            )
        };
        let old = with_mode(json!([{"name": "Open"}, {"name": "Closed"}]));
        let appended = with_mode(json!([{"name": "Open"}, {"name": "Closed"}, {"name": "Paused"}]));
        assert!(diff(&old, &appended).is_empty());
        let inserted = with_mode(json!([{"name": "Paused"}, {"name": "Open"}, {"name": "Closed"}]));
        assert!(levels(&old, &inserted).contains(&Level::Break));
    }

    #[test]
    fn self_referential_types_terminate() {
        // `children: Vec<Node>` and `next: Option<Node>` refer back to
        // `Node`; the comparison must stop instead of recursing forever.
        let node = |value: &str| {
            idl(
                "Node",
                1,
                json!([{"name": "value", "type": value},
                       {"name": "children", "type": {"vec": {"defined": {"name": "Node"}}}},
                       {"name": "next", "type": {"option": {"defined": {"name": "Node"}}}}]),
                json!(null),
            )
        };
        assert!(diff(&node("u64"), &node("u64")).is_empty());
        assert_eq!(levels(&node("u64"), &node("u32")), vec![Level::Break]);
    }

    #[test]
    fn self_referential_account_end_changes_break_nested_values() {
        // Adding or removing a last field is safe for the account itself, but
        // every `Node` stored in `children` changes size too.
        let node = |fields: serde_json::Value| idl("Node", 1, fields, json!(null));
        let value = json!({"name": "value", "type": "u64"});
        let children = json!({"name": "children", "type": {"vec": {"defined": {"name": "Node"}}}});
        let extra = json!({"name": "extra", "type": "u8"});
        let short = node(json!([value, children]));
        let long = node(json!([value, children, extra]));
        assert!(levels(&short, &long).contains(&Level::Break));
        assert!(levels(&long, &short).contains(&Level::Break));
    }
}
