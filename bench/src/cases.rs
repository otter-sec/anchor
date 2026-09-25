#[derive(Clone, Copy)]
pub enum AccountKind {
    Raw,
    Empty,
    Sized,
    Unsized,
    Mint,
    Token,
    Interface,
    Program,
    Signer,
    System,
}

pub struct Case {
    pub name: &'static str,
    pub kind: AccountKind,
    pub init: bool,
    pub counts: &'static [usize],
}

const COUNTS: &[usize] = &[1, 2, 4, 8];
const TOKEN_COUNTS: &[usize] = &[1, 2, 4];

pub const CASES: &[Case] = &[
    Case {
        name: "account_info",
        kind: AccountKind::Raw,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "account_empty",
        kind: AccountKind::Empty,
        init: true,
        counts: COUNTS,
    },
    Case {
        name: "account_sized",
        kind: AccountKind::Sized,
        init: true,
        counts: COUNTS,
    },
    Case {
        name: "account_unsized",
        kind: AccountKind::Unsized,
        init: true,
        counts: COUNTS,
    },
    Case {
        name: "boxed_account_empty",
        kind: AccountKind::Empty,
        init: true,
        counts: COUNTS,
    },
    Case {
        name: "boxed_account_sized",
        kind: AccountKind::Sized,
        init: true,
        counts: COUNTS,
    },
    Case {
        name: "boxed_account_unsized",
        kind: AccountKind::Unsized,
        init: true,
        counts: COUNTS,
    },
    Case {
        name: "boxed_interface_account_mint",
        kind: AccountKind::Mint,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "boxed_interface_account_token",
        kind: AccountKind::Token,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "interface_account_mint",
        kind: AccountKind::Mint,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "interface_account_token",
        kind: AccountKind::Token,
        init: false,
        counts: TOKEN_COUNTS,
    },
    Case {
        name: "interface",
        kind: AccountKind::Interface,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "program",
        kind: AccountKind::Program,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "signer",
        kind: AccountKind::Signer,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "system_account",
        kind: AccountKind::System,
        init: false,
        counts: COUNTS,
    },
    Case {
        name: "unchecked_account",
        kind: AccountKind::Raw,
        init: false,
        counts: COUNTS,
    },
];

pub fn instruction(name: &str, init: bool, count: usize) -> String {
    format!("{name}{}{count}", if init { "_init" } else { "" })
}

pub fn result_name(instruction: &str) -> String {
    let mut uppercase = false;
    instruction
        .chars()
        .filter_map(|character| {
            if character == '_' {
                uppercase = true;
                None
            } else if uppercase {
                uppercase = false;
                Some(character.to_ascii_uppercase())
            } else {
                Some(character)
            }
        })
        .collect()
}

pub fn struct_name(instruction: &str) -> String {
    let camel = result_name(instruction);
    let mut characters = camel.chars();
    characters
        .next()
        .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
        .unwrap_or_default()
}
