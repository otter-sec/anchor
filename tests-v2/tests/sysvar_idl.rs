use {
    anchor_lang::{accounts::SysvarId, IdlAccountType},
    pinocchio::sysvars::{clock::Clock, instructions::Instructions, rent::Rent},
};

#[test]
fn sysvar_idl_addresses_match_well_known_accounts() {
    assert_eq!(
        <Clock as SysvarId>::IDL_ADDRESS,
        "SysvarC1ock11111111111111111111111111111111"
    );
    assert_eq!(
        <Rent as SysvarId>::IDL_ADDRESS,
        "SysvarRent111111111111111111111111111111111"
    );
    assert_eq!(
        <Instructions<&'static [u8]> as SysvarId>::IDL_ADDRESS,
        "Sysvar1nstructions1111111111111111111111111"
    );
}

#[test]
fn sysvar_wrappers_surface_their_idl_address() {
    // End of the chain the IDL builder actually reads:
    // `SysvarId::IDL_ADDRESS` -> `IdlAccountType::__IDL_ADDRESS`.
    use anchor_lang::accounts::{Instructions as AnchorInstructions, Sysvar};

    assert_eq!(
        <Sysvar<Clock> as IdlAccountType>::__IDL_ADDRESS,
        Some("SysvarC1ock11111111111111111111111111111111")
    );
    assert_eq!(
        <Sysvar<Rent> as IdlAccountType>::__IDL_ADDRESS,
        Some("SysvarRent111111111111111111111111111111111")
    );
    assert_eq!(
        <Sysvar<AnchorInstructions> as IdlAccountType>::__IDL_ADDRESS,
        Some("Sysvar1nstructions1111111111111111111111111")
    );
}

#[test]
fn instructions_sysvar_id_is_not_the_system_program() {
    assert_eq!(
        <Instructions<&'static [u8]> as SysvarId>::SYSVAR_ID,
        anchor_lang::address!("Sysvar1nstructions1111111111111111111111111")
    );
    assert_ne!(
        <Instructions<&'static [u8]> as SysvarId>::SYSVAR_ID,
        anchor_lang::address!("11111111111111111111111111111111")
    );
}
