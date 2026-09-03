//! Token-2022 extension CPI helpers.

mod common;

pub mod confidential_transfer;
pub mod confidential_transfer_fee;
pub mod cpi_guard;
pub mod default_account_state;
pub mod group_member_pointer;
pub mod group_pointer;
pub mod immutable_owner;
pub mod interest_bearing_mint;
pub mod memo_transfer;
pub mod metadata_pointer;
pub mod mint_close_authority;
pub mod non_transferable;
pub mod pausable;
pub mod permanent_delegate;
pub mod token_group;
pub mod token_metadata;
pub mod transfer_fee;
pub mod transfer_hook;

pub use {
    cpi_guard::*, default_account_state::*, group_member_pointer::*, group_pointer::*,
    immutable_owner::*, interest_bearing_mint::*, memo_transfer::*, metadata_pointer::*,
    mint_close_authority::*, non_transferable::*, pausable::*, permanent_delegate::*, spl_pod,
    spl_token_metadata_interface, token_group::*, token_metadata::*, transfer_fee::*,
    transfer_hook::*,
};

#[cfg(test)]
mod tests {
    use {
        super::common::validate_token_2022_program,
        anchor_lang::{programs::Token2022, Id},
        pinocchio::address::Address,
        solana_program_error::ProgramError,
    };

    #[test]
    fn token_2022_program_check_accepts_canonical_id() {
        assert_eq!(validate_token_2022_program(&Token2022::id()), Ok(()));
    }

    #[test]
    #[cfg(feature = "guardrails")]
    fn token_2022_program_check_rejects_other_programs() {
        assert_eq!(
            validate_token_2022_program(&Address::new_from_array([1; 32])),
            Err(ProgramError::IncorrectProgramId)
        );
    }
}
