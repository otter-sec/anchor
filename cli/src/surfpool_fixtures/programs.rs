use {
    super::insert,
    crate::{
        bpf_loader_upgradeable,
        config::{Config, TestValidator, WithPath},
        rent_exempt_minimum, resolve_workspace_path, Pubkey, Signer, UpgradeableLoaderState,
        RENT_EPOCH_NEVER,
    },
    anyhow::{bail, Context, Result},
    base64::{engine::general_purpose::STANDARD, Engine},
    serde_json::{json, Map, Value},
    std::fs,
};

fn image(owner: String, data: &[u8], executable: bool) -> Value {
    json!({"owner":owner,"data":STANDARD.encode(data),"lamports":rent_exempt_minimum(data.len() as u64),
        "executable":executable,"rentEpoch":RENT_EPOCH_NEVER,"parsedData":null})
}

pub(super) fn load(
    cfg: &WithPath<Config>,
    test: &TestValidator,
    images: &mut Map<String, Value>,
) -> Result<()> {
    let upgrade_authority = if test
        .genesis
        .iter()
        .flatten()
        .any(|program| program.upgradeable == Some(true))
    {
        cfg.wallet_kp()?.pubkey()
    } else {
        Pubkey::default()
    };
    for program in test.genesis.iter().flatten() {
        let address: Pubkey = program
            .address
            .parse()
            .context("invalid genesis program address")?;
        let path = resolve_workspace_path(cfg, &program.program)?;
        let elf =
            fs::read(&path).with_context(|| format!("Read genesis program {}", path.display()))?;
        if !elf.starts_with(b"\x7fELF") {
            bail!("Genesis program {} is not an ELF file", path.display());
        }
        let loader = bpf_loader_upgradeable::id();
        let (data_address, _) = Pubkey::find_program_address(&[address.as_ref()], &loader);
        let data_state = UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: Some(if program.upgradeable == Some(true) {
                upgrade_authority
            } else {
                // Match Agave's --bpf-program: no user can sign for this authority.
                Pubkey::default()
            }),
        };
        let mut data = bincode::serialize(&data_state)?;
        data.resize(UpgradeableLoaderState::size_of_programdata_metadata(), 0);
        data.extend_from_slice(&elf);
        insert(
            images,
            data_address,
            image(loader.to_string(), &data, false),
        )?;
        let program_data = bincode::serialize(&UpgradeableLoaderState::Program {
            programdata_address: data_address,
        })?;
        insert(
            images,
            address,
            image(loader.to_string(), &program_data, true),
        )?;
    }
    Ok(())
}
