//! Compile declared fixtures into one snapshot before either Surfpool mode starts.
mod accounts;
mod programs;
pub(super) mod reader;
#[cfg(test)]
mod tests;
mod verify;

pub(super) use verify::verify;
use {
    crate::{
        config::{Config, TestValidator, WithPath},
        GeneratedAccount, Pubkey,
    },
    anyhow::{bail, Result},
    serde_json::{Map, Value},
};

fn insert(images: &mut Map<String, Value>, address: Pubkey, image: Value) -> Result<()> {
    let key = address.to_string();
    if images.contains_key(&key) {
        bail!("Duplicate Surfpool fixture account {address}");
    }
    images.insert(key, image);
    Ok(())
}

pub(super) fn compile(
    cfg: &WithPath<Config>,
    test: &Option<TestValidator>,
    generated: &[GeneratedAccount],
) -> Result<Map<String, Value>> {
    let mut images = Map::new();
    if let Some(test) = test {
        if let Some(validator) = &test.validator {
            accounts::load(cfg, validator, &mut images)?;
        }
        programs::load(cfg, test, &mut images)?;
    }
    for account in generated {
        insert(
            &mut images,
            account.pubkey,
            account.surfpool_snapshot_value.clone(),
        )?;
    }
    Ok(images)
}
