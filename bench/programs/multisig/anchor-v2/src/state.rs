use anchor_lang::prelude::*;

pub const MAX_LABEL_LEN: usize = 32;
pub const MAX_SIGNERS: usize = 10;

#[account]
pub struct MultisigConfig {
    pub creator: Address,
    pub threshold: u8,
    pub bump: u8,
    pub label: PodVec<u8, MAX_LABEL_LEN>,
    pub signers: PodVec<Address, MAX_SIGNERS>,
}

impl MultisigConfig {
    pub fn label_str(&self) -> &str {
        // SAFETY: label is UTF-8 validated in set_label/initialize.
        unsafe { core::str::from_utf8_unchecked(self.label.as_slice()) }
    }

    pub fn active_signers(&self) -> &[Address] {
        self.signers.as_slice()
    }
}
