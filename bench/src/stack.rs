use {
    crate::{cases, runner::Runner},
    anyhow::{bail, Context, Result},
    indexmap::IndexMap,
    regex::Regex,
    std::{
        collections::{HashMap, HashSet},
        path::Path,
        process::Command,
    },
};

impl Runner {
    pub fn measure_stack(
        &self,
        platform_tools: &str,
        binary: &Path,
    ) -> Result<IndexMap<String, u64>> {
        let objdump = self
            .bench_dir()
            .join(".cache/avm/platform-tools")
            .join(platform_tools)
            .join("llvm/bin/llvm-objdump");
        if !objdump.is_file() {
            bail!(
                "AVM's platform-tools {platform_tools} install has no llvm-objdump at {}",
                objdump.display()
            );
        }
        let stack = self.output(
            Command::new(&objdump)
                .args(["-s", "-j", ".stack_sizes"])
                .arg(binary),
        )?;
        let symbols = self.output(Command::new(&objdump).args(["-t", "-C"]).arg(binary))?;
        stack_sizes(&stack, &symbols)
    }
}

fn parse_stack_section(output: &str) -> Result<HashMap<u64, u64>> {
    let hex = Regex::new(r"^[0-9a-fA-F]+$")?;
    let mut payload = Vec::new();
    for line in output.lines() {
        for field in line.split_whitespace().skip(1) {
            if field.len() % 2 != 0 || !hex.is_match(field) {
                break;
            }
            for pair in field.as_bytes().as_chunks::<2>().0 {
                payload.push(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?);
            }
        }
    }

    let mut entries = HashMap::new();
    let mut offset = 0;
    while offset + 8 <= payload.len() {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&payload[offset..offset + 8]);
        let mut address = u64::from_le_bytes(bytes);
        offset += 8;
        if address != 0 && address % (1_u64 << 32) == 0 {
            address /= 1_u64 << 32;
        }

        let mut size = 0_u64;
        let mut shift = 0;
        loop {
            let byte = *payload
                .get(offset)
                .context("Truncated ULEB128 stack size")?;
            offset += 1;
            size |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 63 {
                bail!("Invalid ULEB128 value in .stack_sizes");
            }
        }
        entries.insert(address, size);
    }
    Ok(entries)
}

fn parse_symbols(output: &str) -> Result<HashMap<u64, Vec<String>>> {
    let pattern = Regex::new(r"(?i)^([0-9a-f]+)\s+\S+\s+F\s+\.text\s+[0-9a-f]+\s+(.+)$")?;
    let hidden = Regex::new(r"^\.\w+\s+")?;
    let mut symbols = HashMap::<u64, Vec<String>>::new();
    for line in output.lines() {
        let Some(captures) = pattern.captures(line) else {
            continue;
        };
        let address = u64::from_str_radix(&captures[1], 16)?;
        let symbol = hidden.replace(&captures[2], "").into_owned();
        symbols.entry(address).or_default().push(symbol);
    }
    Ok(symbols)
}

fn stack_sizes(stack: &str, symbols: &str) -> Result<IndexMap<String, u64>> {
    let sizes = parse_stack_section(stack)?;
    let symbols = parse_symbols(symbols)?;
    let mut results = IndexMap::new();

    for case in cases::CASES {
        if case.init {
            for &count in case.counts {
                insert_stack_size(
                    &mut results,
                    &sizes,
                    &symbols,
                    &cases::instruction(case.name, true, count),
                )?;
            }
        }
        for &count in case.counts {
            insert_stack_size(
                &mut results,
                &sizes,
                &symbols,
                &cases::instruction(case.name, false, count),
            )?;
        }
    }
    Ok(results)
}

fn insert_stack_size(
    results: &mut IndexMap<String, u64>,
    sizes: &HashMap<u64, u64>,
    symbols: &HashMap<u64, Vec<String>>,
    handler: &str,
) -> Result<()> {
    let name = cases::struct_name(handler);
    let fallback = Regex::new(&format!(r"_5benchNtB\w+_{}{name}I", name.len()))?;
    let legacy_try_accounts = Regex::new(r"::try_accounts::h[0-9a-f]+$")?;
    let matches = sizes
        .iter()
        .filter(|(address, _)| {
            symbols.get(address).is_some_and(|symbols| {
                symbols.iter().any(|symbol| {
                    (symbol.ends_with("::try_accounts")
                        || symbol.ends_with("E12try_accounts")
                        || legacy_try_accounts.is_match(symbol))
                        && (symbol.contains(&format!("<bench::{name} as "))
                            || symbol.contains(&format!("bench::{name} as anchor_lang::Accounts"))
                            || symbol.contains(&format!(
                                "$LT$bench..{name}$u20$as$u20$anchor_lang..Accounts"
                            ))
                            || fallback.is_match(symbol))
                })
            })
        })
        .map(|(_, size)| *size)
        .collect::<HashSet<_>>();
    if matches.len() != 1 {
        bail!("Expected one stack-size entry for {name}::try_accounts, found {matches:?}");
    }
    let size = *matches.iter().next().unwrap();
    results.insert(handler.to_owned(), size);
    Ok(())
}
