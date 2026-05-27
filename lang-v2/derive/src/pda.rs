//! Macro-time PDA bump precomputation.
//!
//! When `#[account(seeds = [..], bump)]` has seeds that are all byte
//! literals, the proc macro derives the bump at expansion time and emits
//! it as a const. Falls back to the runtime path for non-literal seeds
//! or when program id discovery fails.
//!
//! Program id discovery walks the `CARGO_MANIFEST_DIR`'s `src/lib.rs`
//! looking for a top-level `declare_id!("...")` macro invocation and
//! base58-decodes the literal. The result is cached in a thread-local so
//! multi-`#[derive(Accounts)]` crates don't re-parse the file for each
//! derive.

use {
    sha2::{Digest, Sha256},
    std::cell::RefCell,
};

const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";
const MAX_PDA_SEEDS: usize = 16;
const MAX_PDA_SEED_LEN: usize = 32;

thread_local! {
    /// `None`   = not yet attempted.
    /// `Some(x)` = attempted once; `x` is the discovery result.
    static CACHED_PROGRAM_ID: RefCell<Option<Option<[u8; 32]>>> = const { RefCell::new(None) };
}

/// Returns the program id declared in the current crate's `src/lib.rs`,
/// or `None` if it can't be discovered (file missing, parse failure, no
/// `declare_id!` macro, malformed argument, bad base58 — any failure
/// silently produces `None`).
pub(crate) fn discover_program_id() -> Option<[u8; 32]> {
    CACHED_PROGRAM_ID.with(|cell| {
        if let Some(cached) = *cell.borrow() {
            return cached;
        }
        let id = try_discover_program_id();
        *cell.borrow_mut() = Some(id);
        id
    })
}

fn try_discover_program_id() -> Option<[u8; 32]> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let lib_rs = std::path::PathBuf::from(manifest_dir)
        .join("src")
        .join("lib.rs");
    let source = std::fs::read_to_string(&lib_rs).ok()?;
    let file = syn::parse_file(&source).ok()?;

    for item in &file.items {
        if let syn::Item::Macro(item_macro) = item {
            let last = item_macro.mac.path.segments.last()?;
            if last.ident != "declare_id" {
                continue;
            }
            let lit: syn::LitStr = syn::parse2(item_macro.mac.tokens.clone()).ok()?;
            let decoded = bs58::decode(lit.value()).into_vec().ok()?;
            if decoded.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&decoded);
                return Some(arr);
            }
        }
    }
    None
}

/// Try to extract a single seed expression as constant bytes. Handles
/// `b"literal"`, `"literal"`, `[1, 2, 3]`, `"lit".as_bytes()`.
pub(crate) fn seed_as_const_bytes(expr: &syn::Expr) -> Option<Vec<u8>> {
    // Peel &ref wrappers
    let mut cur = expr;
    while let syn::Expr::Reference(r) = cur {
        cur = &r.expr;
    }
    match cur {
        syn::Expr::Lit(syn::ExprLit { lit, .. }) => match lit {
            syn::Lit::ByteStr(b) => Some(b.value()),
            syn::Lit::Str(s) => Some(s.value().into_bytes()),
            syn::Lit::Byte(b) => Some(vec![b.value()]),
            _ => None,
        },
        syn::Expr::Array(arr) => arr
            .elems
            .iter()
            .map(|e| {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(i),
                    ..
                }) = e
                {
                    i.base10_parse::<u8>().ok()
                } else {
                    None
                }
            })
            .collect(),
        syn::Expr::MethodCall(mc) if mc.method == "as_bytes" && mc.args.is_empty() => {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &*mc.receiver
            {
                Some(s.value().into_bytes())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// If every element of `seeds` is a byte-string literal (`b"..."`), return
/// their decoded contents in order. Any non-literal seed (a field ref, a
/// method call, a path expression) returns `None`, signalling that the
/// optimization can't apply for this field.
pub(crate) fn seeds_as_byte_literals(seeds: &[&syn::Expr]) -> Option<Vec<Vec<u8>>> {
    seeds
        .iter()
        .map(|expr| match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::ByteStr(b),
                ..
            }) => Some(b.value()),
            _ => None,
        })
        .collect()
}

/// Host-side re-implementation of `find_program_address`. Iterates bumps
/// from `255` down to `0`, hashing `[seeds..., bump, program_id,
/// PDA_MARKER]` with sha256, returning the first bump whose hash decodes
/// to an off-curve Edwards25519 point — along with that hash, which is
/// the canonical PDA address.
///
/// Returns both the PDA address and bump — the address is emitted as a
/// const for runtime comparison, the bump is needed for init CPI seeds.
///
/// Matches the algorithm in `anchor_lang_v2::cpi::find_program_address`,
/// just running on the host via `sha2` + `curve25519-dalek` instead of
/// the sbpf syscalls.
///
/// Returns `None` if no valid bump exists (cryptographically ~2^-256, so
/// effectively unreachable).
pub(crate) fn precompute_pda(seeds: &[&[u8]], program_id: &[u8; 32]) -> Option<(u8, [u8; 32])> {
    use curve25519_dalek::edwards::CompressedEdwardsY;

    if seeds.len() >= MAX_PDA_SEEDS || seeds.iter().any(|seed| seed.len() > MAX_PDA_SEED_LEN) {
        return None;
    }

    let mut bump: i32 = u8::MAX as i32;
    while bump >= 0 {
        let mut hasher = Sha256::new();
        for s in seeds {
            hasher.update(s);
        }
        hasher.update([bump as u8]);
        hasher.update(program_id);
        hasher.update(PDA_MARKER);
        let hash: [u8; 32] = hasher.finalize().into();

        // PDAs are the sha256 outputs that do NOT decompress to a valid
        // Edwards point (i.e. are off-curve). `.decompress()` returns
        // `None` exactly when the candidate is off-curve — that's the
        // condition we want.
        if CompressedEdwardsY(hash).decompress().is_none() {
            return Some((bump as u8, hash));
        }
        bump -= 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-value test: `find_program_address(&[b""], DHvAxn78...)` must
    /// return bump = 253. Verified against cargo-expand output of the
    /// anchor-v2-testing `CheckEmptySeedPda` struct, which uses exactly
    /// this seed + program id pair.
    #[test]
    fn precompute_pda_matches_reference_empty_seed() {
        // DHvAxn78uSi8YsUoVBVU13CWY24fDCecSRdd7u1tLtz2 decoded to 32 bytes.
        let program_id = bs58::decode("DHvAxn78uSi8YsUoVBVU13CWY24fDCecSRdd7u1tLtz2")
            .into_vec()
            .expect("valid base58");
        assert_eq!(program_id.len(), 32);
        let mut pid = [0u8; 32];
        pid.copy_from_slice(&program_id);

        let (bump, pda) = precompute_pda(&[b""], &pid).expect("bump should exist");
        assert_eq!(bump, 253);
        // PDA must be a non-zero 32-byte hash.
        assert!(pda.iter().any(|b| *b != 0));
    }

    #[test]
    fn precompute_pda_rejects_seed_count_that_leaves_no_bump_slot() {
        let program_id = [7u8; 32];
        let seeds = [&[][..]; MAX_PDA_SEEDS];

        assert!(precompute_pda(&seeds, &program_id).is_none());
    }

    #[test]
    fn precompute_pda_rejects_oversized_seed() {
        let program_id = [7u8; 32];
        let long = [0u8; MAX_PDA_SEED_LEN + 1];
        let seeds = [long.as_slice()];

        assert!(precompute_pda(&seeds, &program_id).is_none());
    }

    #[test]
    fn seeds_as_byte_literals_accepts_all_literals() {
        let seeds: Vec<syn::Expr> = vec![
            syn::parse_str(r#"b"foo""#).unwrap(),
            syn::parse_str(r#"b"bar""#).unwrap(),
        ];
        let refs: Vec<&syn::Expr> = seeds.iter().collect();
        let decoded = seeds_as_byte_literals(&refs).expect("all literal");
        assert_eq!(decoded, vec![b"foo".to_vec(), b"bar".to_vec()]);
    }

    #[test]
    fn seeds_as_byte_literals_rejects_mixed() {
        let seeds: Vec<syn::Expr> = vec![
            syn::parse_str(r#"b"foo""#).unwrap(),
            syn::parse_str("wallet").unwrap(),
        ];
        let refs: Vec<&syn::Expr> = seeds.iter().collect();
        assert!(seeds_as_byte_literals(&refs).is_none());
    }

    #[test]
    fn seeds_as_byte_literals_rejects_method_call() {
        let seeds: Vec<syn::Expr> = vec![
            syn::parse_str(r#"b"vault""#).unwrap(),
            syn::parse_str("config.address().as_ref()").unwrap(),
        ];
        let refs: Vec<&syn::Expr> = seeds.iter().collect();
        assert!(seeds_as_byte_literals(&refs).is_none());
    }
}
