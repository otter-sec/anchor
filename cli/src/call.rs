//! `anchor call`: build an instruction from the IDL, fill its accounts, then simulate it (or send
//! it with `--send`) and print the result with the IDL's names.

use {
    crate::{
        compat::{solana_pubkey, solana_transaction},
        config::{Config, ConfigOverride},
        convert_idl, create_client, deserialize_idl_type_to_json, get_cluster_and_wallet,
        get_keypair,
    },
    anchor_lang_idl::types::{
        Idl, IdlArrayLen, IdlDefinedFields, IdlInstruction, IdlInstructionAccount,
        IdlInstructionAccountItem, IdlSeed, IdlType, IdlTypeDefTy,
    },
    anyhow::{anyhow, bail, Context, Result},
    base64::Engine,
    serde_json::Value as JsonValue,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::{collections::BTreeMap, fs, path::PathBuf, str::FromStr},
};

pub struct CallArgs {
    /// `<program>.<instruction>`
    pub target: String,
    /// Instruction arguments as a JSON object keyed by argument name.
    pub args: Option<String>,
    /// `name=<pubkey>` or `name=<keypair file>` (the keypair also signs).
    pub accounts: Vec<String>,
    pub idl: Option<PathBuf>,
    pub send: bool,
}

/// How an account address was found; printed next to each account.
enum Source {
    Given,
    Signer,
    Wallet,
    Address,
    Pda(String),
    OptionalNone,
}

pub fn call(cfg_override: &ConfigOverride, a: CallArgs) -> Result<()> {
    let (program_name, ix_name) = a
        .target
        .split_once('.')
        .ok_or_else(|| anyhow!("Use the format <program>.<instruction>, e.g. counter.increment"))?;
    let idl = load_idl(cfg_override, program_name, a.idl.as_ref())?;
    let program_id = Pubkey::from_str(&idl.address)?;
    let ix = idl
        .instructions
        .iter()
        .find(|i| i.name == ix_name || i.name == heck::ToSnakeCase::to_snake_case(ix_name))
        .ok_or_else(|| {
            let names: Vec<_> = idl.instructions.iter().map(|i| i.name.as_str()).collect();
            anyhow!(
                "Instruction '{ix_name}' not found in '{program_name}'. Available: {}",
                names.join(", ")
            )
        })?;

    // Arguments
    let args: JsonValue = match &a.args {
        Some(s) => serde_json::from_str(s).context("--args must be a JSON object")?,
        None => JsonValue::Object(Default::default()),
    };
    let mut data = ix.discriminator.clone();
    for field in &ix.args {
        let value = args.get(&field.name).ok_or_else(|| {
            anyhow!(
                "Missing argument '{}' ({}). Pass it in --args, e.g. --args '{{\"{}\": ...}}'",
                field.name,
                type_name(&field.ty),
                field.name
            )
        })?;
        encode(&field.ty, value, &idl, &mut data)
            .with_context(|| format!("argument '{}'", field.name))?;
    }

    // Accounts
    let (url, wallet_path) = get_cluster_and_wallet(cfg_override)?;
    let url = cfg_override
        .cluster
        .as_ref()
        .map(|c| c.url().to_string())
        .unwrap_or(url);
    let wallet = get_keypair(&PathBuf::from(&wallet_path))?;
    let mut extra_signers: Vec<Keypair> = vec![];
    // name -> (key, given as a keypair file)
    let mut given: BTreeMap<String, (Pubkey, bool)> = BTreeMap::new();
    for item in &a.accounts {
        let (name, value) = item
            .split_once('=')
            .ok_or_else(|| anyhow!("--account takes name=<pubkey or keypair file>"))?;
        let entry = match Pubkey::from_str(value) {
            Ok(k) => (k, false),
            Err(_) => {
                let kp = get_keypair(&PathBuf::from(value))?;
                let k = kp.pubkey();
                extra_signers.push(kp);
                (k, true)
            }
        };
        given.insert(name.to_string(), entry);
    }

    let flat = flatten(&ix.accounts, "");
    let resolved = resolve_accounts(&flat, &given, &args, ix, &idl, &program_id, &wallet)?;

    println!("{}.{} on {}", program_name, ix.name, url);
    println!("Accounts:");
    let width = flat.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    let mut metas = vec![];
    for (name, acc) in &flat {
        let (key, source) = &resolved[name];
        let flags = match (acc.signer, acc.writable) {
            (true, true) => "signer, writable",
            (true, false) => "signer",
            (false, true) => "writable",
            (false, false) => "",
        };
        let how = match source {
            Source::Given => "given".to_string(),
            Source::Signer => "given keypair".to_string(),
            Source::Wallet => "wallet".to_string(),
            Source::Address => "fixed address".to_string(),
            Source::Pda(s) => format!("PDA {s}"),
            Source::OptionalNone => "optional, not set".to_string(),
        };
        println!("  {name:width$}  {key}  {flags:16}  ({how})");
        metas.push(if acc.writable {
            AccountMeta::new(*key, acc.signer)
        } else {
            AccountMeta::new_readonly(*key, acc.signer)
        });
    }

    let instruction = Instruction {
        program_id,
        accounts: metas,
        data,
    };
    let client = create_client(&url);
    let mut signers: Vec<&Keypair> = vec![&wallet];
    signers.extend(extra_signers.iter());
    let blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&wallet.pubkey()),
        &signers,
        blockhash,
    );

    let sim = client.simulate_transaction(&tx)?.value;
    let logs = sim.logs.unwrap_or_default();
    match &sim.err {
        None => println!(
            "\nSimulation: ok, {} compute units",
            sim.units_consumed.unwrap_or_default()
        ),
        Some(err) => {
            let err = format!("{err:?}");
            println!("\nSimulation failed: {}", explain_error(&err, &idl, &logs));
        }
    }
    println!("Logs:");
    for l in &logs {
        println!("  {l}");
    }
    if let (Some(ret), Some(ty)) = (&sim.return_data, &ix.returns) {
        let bytes = base64::engine::general_purpose::STANDARD.decode(&ret.data.0)?;
        let value = deserialize_idl_type_to_json(ty, &mut bytes.as_slice(), &idl)?;
        println!("Returned: {value}");
    }
    if sim.err.is_some() {
        bail!("simulation failed; nothing was sent");
    }
    if a.send {
        let sig = client.send_and_confirm_transaction(&tx)?;
        println!("\nSent: {sig}");
    } else {
        println!("\nNot sent (simulation only). Add --send to submit it.");
    }
    Ok(())
}

fn load_idl(cfg_override: &ConfigOverride, program: &str, path: Option<&PathBuf>) -> Result<Idl> {
    if let Some(p) = path {
        return convert_idl(&fs::read(p)?);
    }
    let cfg = Config::discover(cfg_override)?
        .ok_or_else(|| anyhow!("Run inside an Anchor workspace or pass --idl <file>"))?;
    let programs = cfg.read_all_programs()?;
    let p = programs
        .iter()
        .find(|p| p.lib_name == program)
        .ok_or_else(|| anyhow!("Program '{program}' not found in the workspace"))?;
    p.idl
        .clone()
        .ok_or_else(|| anyhow!("No IDL for '{program}'. Run `anchor build` first."))
}

fn flatten<'a>(
    items: &'a [IdlInstructionAccountItem],
    prefix: &str,
) -> Vec<(String, &'a IdlInstructionAccount)> {
    let mut out = vec![];
    for item in items {
        match item {
            IdlInstructionAccountItem::Single(a) => out.push((format!("{prefix}{}", a.name), a)),
            IdlInstructionAccountItem::Composite(c) => {
                out.extend(flatten(&c.accounts, &format!("{prefix}{}.", c.name)))
            }
        }
    }
    out
}

fn resolve_accounts(
    flat: &[(String, &IdlInstructionAccount)],
    given: &BTreeMap<String, (Pubkey, bool)>,
    args: &JsonValue,
    ix: &IdlInstruction,
    idl: &Idl,
    program_id: &Pubkey,
    wallet: &Keypair,
) -> Result<BTreeMap<String, (Pubkey, Source)>> {
    let mut out: BTreeMap<String, (Pubkey, Source)> = BTreeMap::new();
    for (name, acc) in flat {
        if let Some((k, keypair)) = given.get(name).or_else(|| given.get(&acc.name)) {
            let src = if *keypair {
                Source::Signer
            } else {
                Source::Given
            };
            out.insert(name.clone(), (*k, src));
        } else if let Some(addr) = &acc.address {
            out.insert(name.clone(), (Pubkey::from_str(addr)?, Source::Address));
        }
    }
    // Seeds may refer to other accounts: repeat until nothing new resolves.
    loop {
        let mut progress = false;
        for (name, acc) in flat {
            if out.contains_key(name) {
                continue;
            }
            if let Some(pda) = &acc.pda {
                if let Some((key, desc)) = derive_pda(pda, &out, args, ix, idl, program_id)? {
                    out.insert(name.clone(), (key, Source::Pda(desc)));
                    progress = true;
                }
            }
        }
        if !progress {
            break;
        }
    }
    let mut missing = vec![];
    for (name, acc) in flat {
        if out.contains_key(name) {
            continue;
        }
        if acc.signer && acc.pda.is_none() {
            out.insert(name.clone(), (wallet.pubkey(), Source::Wallet));
        } else if acc.optional {
            out.insert(name.clone(), (*program_id, Source::OptionalNone));
        } else {
            missing.push(name.clone());
        }
    }
    if let Some(name) = missing.first() {
        let acc = flat.iter().find(|(n, _)| n == name).unwrap().1;
        let why = if !acc.relations.is_empty() {
            " (reading it from another account's data is not supported yet)"
        } else if acc.pda.is_some() {
            " (its PDA seeds use account data, which is not supported yet)"
        } else {
            ""
        };
        bail!(
            "Cannot fill account(s) {}{why}. Pass them with --account <name>=<pubkey>",
            missing.join(", ")
        );
    }
    Ok(out)
}

/// Returns None while a seed still depends on an unresolved account.
fn derive_pda(
    pda: &anchor_lang_idl::types::IdlPda,
    resolved: &BTreeMap<String, (Pubkey, Source)>,
    args: &JsonValue,
    ix: &IdlInstruction,
    idl: &Idl,
    program_id: &Pubkey,
) -> Result<Option<(Pubkey, String)>> {
    let mut seeds: Vec<Vec<u8>> = vec![];
    let mut desc: Vec<String> = vec![];
    for seed in &pda.seeds {
        match seed {
            IdlSeed::Const(c) => {
                desc.push(match std::str::from_utf8(&c.value) {
                    Ok(s) if s.chars().all(|ch| !ch.is_control()) => format!("\"{s}\""),
                    _ => format!("{:?}", c.value),
                });
                seeds.push(c.value.clone());
            }
            IdlSeed::Arg(s) => {
                let mut parts = s.path.split('.');
                let arg_name = parts.next().unwrap();
                let field = ix
                    .args
                    .iter()
                    .find(|f| f.name == arg_name)
                    .ok_or_else(|| anyhow!("seed uses unknown argument '{arg_name}'"))?;
                if parts.next().is_some() {
                    bail!(
                        "PDA seed from a nested argument field ({}) is not supported yet",
                        s.path
                    );
                }
                let value = args
                    .get(arg_name)
                    .ok_or_else(|| anyhow!("seed needs argument '{arg_name}'"))?;
                seeds.push(seed_bytes(&field.ty, value, idl)?);
                desc.push(format!("arg {arg_name}"));
            }
            IdlSeed::Account(s) => {
                if s.path.contains('.') {
                    return Ok(None); // field of account data: not supported in this version
                }
                match resolved.get(&s.path) {
                    Some((k, _)) => {
                        seeds.push(k.to_bytes().to_vec());
                        desc.push(s.path.clone());
                    }
                    None => return Ok(None),
                }
            }
        }
    }
    let owner = match &pda.program {
        None => *program_id,
        Some(IdlSeed::Const(c)) => Pubkey::try_from(c.value.as_slice())
            .map_err(|_| anyhow!("PDA program seed is not a pubkey"))?,
        Some(IdlSeed::Account(s)) => match resolved.get(&s.path) {
            Some((k, _)) => *k,
            None => return Ok(None),
        },
        Some(IdlSeed::Arg(_)) => bail!("PDA program from an argument is not supported yet"),
    };
    let refs: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
    let (key, _) = Pubkey::find_program_address(&refs, &owner);
    Ok(Some((key, format!("[{}]", desc.join(", ")))))
}

/// Seed bytes follow Anchor's `seeds = [...]` convention: raw bytes for strings and byte
/// vectors, Borsh for everything else.
fn seed_bytes(ty: &IdlType, value: &JsonValue, idl: &Idl) -> Result<Vec<u8>> {
    match ty {
        IdlType::String => Ok(value
            .as_str()
            .ok_or_else(|| anyhow!("expected a string"))?
            .as_bytes()
            .to_vec()),
        IdlType::Bytes => {
            let mut out = vec![];
            encode(ty, value, idl, &mut out)?;
            Ok(out[4..].to_vec())
        }
        _ => {
            let mut out = vec![];
            encode(ty, value, idl, &mut out)?;
            Ok(out)
        }
    }
}

fn int<T: FromStr>(v: &JsonValue) -> Result<T> {
    let s = match v {
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => s.clone(),
        _ => bail!("expected a number, got {v}"),
    };
    s.parse::<T>().map_err(|_| anyhow!("'{s}' is out of range"))
}

/// Borsh-encodes a JSON value as the given IDL type.
fn encode(ty: &IdlType, v: &JsonValue, idl: &Idl, out: &mut Vec<u8>) -> Result<()> {
    match ty {
        IdlType::Bool => out.push(v.as_bool().ok_or_else(|| anyhow!("expected true/false"))? as u8),
        IdlType::U8 => out.extend(int::<u8>(v)?.to_le_bytes()),
        IdlType::I8 => out.extend(int::<i8>(v)?.to_le_bytes()),
        IdlType::U16 => out.extend(int::<u16>(v)?.to_le_bytes()),
        IdlType::I16 => out.extend(int::<i16>(v)?.to_le_bytes()),
        IdlType::U32 => out.extend(int::<u32>(v)?.to_le_bytes()),
        IdlType::I32 => out.extend(int::<i32>(v)?.to_le_bytes()),
        IdlType::U64 => out.extend(int::<u64>(v)?.to_le_bytes()),
        IdlType::I64 => out.extend(int::<i64>(v)?.to_le_bytes()),
        IdlType::U128 => out.extend(int::<u128>(v)?.to_le_bytes()),
        IdlType::I128 => out.extend(int::<i128>(v)?.to_le_bytes()),
        IdlType::F32 => out.extend((int::<f64>(v)? as f32).to_le_bytes()),
        IdlType::F64 => out.extend(int::<f64>(v)?.to_le_bytes()),
        IdlType::String => {
            let s = v.as_str().ok_or_else(|| anyhow!("expected a string"))?;
            out.extend((s.len() as u32).to_le_bytes());
            out.extend(s.as_bytes());
        }
        IdlType::Pubkey => {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow!("expected a base58 pubkey"))?;
            out.extend(Pubkey::from_str(s)?.to_bytes());
        }
        IdlType::Bytes => {
            let items = v
                .as_array()
                .ok_or_else(|| anyhow!("expected an array of bytes"))?;
            out.extend((items.len() as u32).to_le_bytes());
            for i in items {
                out.push(int::<u8>(i)?);
            }
        }
        IdlType::Option(inner) => {
            if v.is_null() {
                out.push(0);
            } else {
                out.push(1);
                encode(inner, v, idl, out)?;
            }
        }
        IdlType::Vec(inner) => {
            let items = v.as_array().ok_or_else(|| anyhow!("expected an array"))?;
            out.extend((items.len() as u32).to_le_bytes());
            for i in items {
                encode(inner, i, idl, out)?;
            }
        }
        IdlType::Array(inner, IdlArrayLen::Value(n)) => {
            let items = v.as_array().ok_or_else(|| anyhow!("expected an array"))?;
            if items.len() != *n {
                bail!("expected {n} items, got {}", items.len());
            }
            for i in items {
                encode(inner, i, idl, out)?;
            }
        }
        IdlType::Defined { name, generics } if generics.is_empty() => {
            let def = idl
                .types
                .iter()
                .find(|t| &t.name == name)
                .ok_or_else(|| anyhow!("type '{name}' not in IDL"))?;
            match &def.ty {
                IdlTypeDefTy::Struct { fields } => encode_fields(fields.as_ref(), v, idl, out)?,
                IdlTypeDefTy::Enum { variants } => {
                    let (vname, inner) = match v {
                        JsonValue::String(s) => (s.as_str(), &JsonValue::Null),
                        JsonValue::Object(m) if m.len() == 1 => {
                            let (k, inner) = m.iter().next().unwrap();
                            (k.as_str(), inner)
                        }
                        _ => bail!("expected \"Variant\" or {{\"Variant\": ...}} for enum {name}"),
                    };
                    let idx = variants
                        .iter()
                        .position(|x| x.name == vname)
                        .ok_or_else(|| anyhow!("'{vname}' is not a variant of {name}"))?;
                    out.push(idx as u8);
                    encode_fields(variants[idx].fields.as_ref(), inner, idl, out)?;
                }
                IdlTypeDefTy::Type { alias } => encode(alias, v, idl, out)?,
            }
        }
        other => bail!("{} arguments are not supported yet", type_name(other)),
    }
    Ok(())
}

fn encode_fields(
    fields: Option<&IdlDefinedFields>,
    v: &JsonValue,
    idl: &Idl,
    out: &mut Vec<u8>,
) -> Result<()> {
    match fields {
        None => Ok(()),
        Some(IdlDefinedFields::Named(fs)) => {
            for f in fs {
                let fv = v
                    .get(&f.name)
                    .ok_or_else(|| anyhow!("missing field '{}'", f.name))?;
                encode(&f.ty, fv, idl, out).with_context(|| format!("field '{}'", f.name))?;
            }
            Ok(())
        }
        Some(IdlDefinedFields::Tuple(ts)) => {
            let items = v.as_array().ok_or_else(|| anyhow!("expected an array"))?;
            if items.len() != ts.len() {
                bail!("expected {} items, got {}", ts.len(), items.len());
            }
            for (t, i) in ts.iter().zip(items) {
                encode(t, i, idl, out)?;
            }
            Ok(())
        }
    }
}

fn type_name(ty: &IdlType) -> String {
    serde_json::to_string(ty).unwrap_or_else(|_| format!("{ty:?}"))
}

/// Names a custom program error from the IDL, and adds the AnchorError log line when present.
fn explain_error(err: &str, idl: &Idl, logs: &[String]) -> String {
    let code = err
        .split("Custom(")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.parse::<u32>().ok());
    let anchor_line = logs
        .iter()
        .find(|l| l.contains("AnchorError"))
        .map(|l| l.trim_start_matches("Program log: ").to_string());
    match (code, anchor_line) {
        (_, Some(line)) => format!("{err}\n  {line}"),
        (Some(c), None) => match idl.errors.iter().find(|e| e.code == c) {
            Some(e) => format!(
                "{err}\n  {} ({}): {}",
                e.name,
                c,
                e.msg.clone().unwrap_or_default()
            ),
            None => err.to_string(),
        },
        _ => err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, anchor_lang::AnchorSerialize, serde_json::json};

    #[derive(AnchorSerialize)]
    enum Mode {
        Open,
        Limited { max: u16 },
    }

    #[derive(AnchorSerialize)]
    struct Params {
        amount: u64,
        name: String,
        owner: Pubkey,
        memo: Option<String>,
        tags: Vec<u8>,
        ratio: [i32; 2],
        mode: Mode,
        big: u128,
    }

    fn idl() -> Idl {
        serde_json::from_value(json!({
            "address": "11111111111111111111111111111111",
            "metadata": {"name": "t", "version": "0.1.0", "spec": "0.1.0"},
            "instructions": [], "accounts": [], "errors": [],
            "types": [
                {"name": "Mode", "type": {"kind": "enum", "variants": [
                    {"name": "Open"},
                    {"name": "Limited", "fields": [{"name": "max", "type": "u16"}]}
                ]}},
                {"name": "Params", "type": {"kind": "struct", "fields": [
                    {"name": "amount", "type": "u64"},
                    {"name": "name", "type": "string"},
                    {"name": "owner", "type": "pubkey"},
                    {"name": "memo", "type": {"option": "string"}},
                    {"name": "tags", "type": {"vec": "u8"}},
                    {"name": "ratio", "type": {"array": ["i32", 2]}},
                    {"name": "mode", "type": {"defined": {"name": "Mode"}}},
                    {"name": "big", "type": "u128"}
                ]}}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn encodes_like_borsh() {
        let owner = Pubkey::new_unique();
        let idl = idl();
        let ty = IdlType::Defined {
            name: "Params".into(),
            generics: vec![],
        };
        for (value, expected) in [
            (
                json!({"amount": 5, "name": "a", "owner": owner.to_string(), "memo": null,
                       "tags": [1, 2], "ratio": [-1, 7], "mode": "Open",
                       "big": "340282366920938463463374607431768211455"}),
                Params {
                    amount: 5,
                    name: "a".into(),
                    owner,
                    memo: None,
                    tags: vec![1, 2],
                    ratio: [-1, 7],
                    mode: Mode::Open,
                    big: u128::MAX,
                },
            ),
            (
                json!({"amount": "18446744073709551615", "name": "", "owner": owner.to_string(),
                       "memo": "hi", "tags": [], "ratio": [0, 0],
                       "mode": {"Limited": {"max": 9}}, "big": 1}),
                Params {
                    amount: u64::MAX,
                    name: "".into(),
                    owner,
                    memo: Some("hi".into()),
                    tags: vec![],
                    ratio: [0, 0],
                    mode: Mode::Limited { max: 9 },
                    big: 1,
                },
            ),
        ] {
            let mut out = vec![];
            encode(&ty, &value, &idl, &mut out).unwrap();
            let mut want = vec![];
            expected.serialize(&mut want).unwrap();
            assert_eq!(out, want);
        }
    }

    #[test]
    fn rejects_bad_values() {
        let idl = idl();
        let mut out = vec![];
        assert!(encode(&IdlType::U8, &json!(256), &idl, &mut out).is_err());
        assert!(encode(&IdlType::U64, &json!(-1), &idl, &mut out).is_err());
        assert!(encode(&IdlType::Pubkey, &json!("nope"), &idl, &mut out).is_err());
        let mode = IdlType::Defined {
            name: "Mode".into(),
            generics: vec![],
        };
        assert!(encode(&mode, &json!("Closed"), &idl, &mut out).is_err());
    }

    #[test]
    fn seed_bytes_match_anchor_seeds() {
        let idl = idl();
        assert_eq!(
            seed_bytes(&IdlType::String, &json!("abc"), &idl).unwrap(),
            b"abc"
        );
        assert_eq!(
            seed_bytes(&IdlType::U64, &json!(7), &idl).unwrap(),
            7u64.to_le_bytes()
        );
        assert_eq!(
            seed_bytes(&IdlType::Bytes, &json!([1, 2]), &idl).unwrap(),
            vec![1, 2]
        );
    }
}
