use anchor_lang::prelude::*;

use crate::{
    errors::MultisigError,
    state::{MultisigConfig, MAX_LABEL_LEN},
};

#[derive(Accounts)]
pub struct SetLabel {
    #[account(mut)]
    pub creator: Signer,
    #[account(
        mut,
        has_one = creator,
        seeds = [b"multisig", creator.address().as_ref()],
        bump = config.bump
    )]
    pub config: Account<MultisigConfig>,
}

impl SetLabel {
    #[inline(always)]
    pub fn update_label(&mut self, label_len: u8, label: [u8; 32]) -> Result<()> {
        let len = label_len as usize;
        if len > MAX_LABEL_LEN {
            return Err(MultisigError::LabelTooLong.into());
        }
        // UTF-8 validate for parity with frameworks that deserialize as `&str`.
        core::str::from_utf8(&label[..len])
            .map_err(|_| MultisigError::LabelTooLong)?;
        self.config.label.set_from_slice(&label[..len]);
        Ok(())
    }
}
