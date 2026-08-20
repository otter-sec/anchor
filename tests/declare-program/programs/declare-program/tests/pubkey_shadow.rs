use anchor_lang::prelude::*;

declare_program!(pubkey_shadow);

#[test]
fn builtin_pubkey_types_are_not_shadowed_by_idl_names() {
    let holder = pubkey_shadow::types::Holder {
        key: anchor_lang::prelude::Pubkey::default(),
    };
    let _: anchor_lang::prelude::Pubkey = holder.key;

    let _ = pubkey_shadow::client::args::AcceptHolder { holder };
}
