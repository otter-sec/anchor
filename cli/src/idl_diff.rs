//! `anchor idl diff`: compare the account layouts of two IDLs and report changes that would make
//! the new program misread accounts written by the old one.

use {
    anchor_lang_idl::types::{
        Idl, IdlArrayLen, IdlDefinedFields, IdlField, IdlType, IdlTypeDef, IdlTypeDefTy,
    },
    std::{collections::HashSet, fmt},
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
        let (Some(old_def), Some(new_def)) = (find(old, &acc.name), find(new, &new_acc.name))
        else {
            continue;
        };
        if old_def.serialization != new_def.serialization || old_def.repr != new_def.repr {
            push(
                Level::Break,
                "serialization or repr changed: existing bytes are read with a different encoding"
                    .into(),
            );
            continue;
        }
        let mut cx = Cx {
            old,
            new,
            findings: vec![],
            visiting: HashSet::from([(acc.name.clone(), new_acc.name.clone())]),
        };
        cx.compare_def(&old_def.ty, &new_def.ty, &acc.name, true);
        for (level, message) in cx.findings {
            push(level, message);
        }
    }
    out
}

fn find<'a>(idl: &'a Idl, name: &str) -> Option<&'a IdlTypeDef> {
    idl.types.iter().find(|t| t.name == name)
}

struct Cx<'a> {
    old: &'a Idl,
    new: &'a Idl,
    findings: Vec<(Level, String)>,
    /// Pairs of defined types being compared, to stop on recursive types
    /// such as `Vec<Self>` or `Option<Box<Self>>`.
    visiting: HashSet<(String, String)>,
}

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
                    inner
                        .into_iter()
                        .map(|(_, m)| m)
                        .next()
                        .unwrap_or_else(|| format!("{p}: changed"))
                };
                let message = if detail.contains("change meaning") {
                    detail
                } else {
                    format!(
                        "{detail}; this field and the ones after it are read at the wrong offset, \
                         so existing accounts decode into wrong values"
                    )
                };
                self.add(Level::Break, message);
                return false;
            }
            if na != nb {
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
                     reallocated (`realloc` or `Migration<From, To>`), unless they were created \
                     with spare space",
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
                if ga != gb {
                    self.add(Level::Break, format!("{path}: generic arguments changed"));
                    return false;
                }
                match (find(self.old, na), find(self.new, nb)) {
                    (Some(da), Some(db)) => {
                        // Already compared further up: its changes are
                        // reported there.
                        let pair = (na.clone(), nb.clone());
                        if !self.visiting.insert(pair.clone()) {
                            return true;
                        }
                        let same = self.compare_def(&da.ty, &db.ty, path, false);
                        self.visiting.remove(&pair);
                        same
                    }
                    _ => {
                        if na != nb {
                            self.add(Level::Break, format!("{path}: type `{na}` -> `{nb}`"));
                            return false;
                        }
                        true
                    }
                }
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
        // `children: Vec<Node>` and `next: Option<Box<Node>>` refer back to
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
}
