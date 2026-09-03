use {
    anchor_lang_idl::{
        convert::convert_idl,
        types::{
            IdlDefinedFields, IdlInstructionAccountItem, IdlSeed, IdlType, IdlTypeDefTy, IDL_SPEC,
        },
    },
    serde_json::json,
    sha2::{Digest, Sha256},
};

fn discriminator(prefix: &str, name: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(b":");
    hasher.update(name);
    hasher.finalize()[..8].into()
}

fn convert_json(value: serde_json::Value) -> anchor_lang_idl::types::Idl {
    let bytes = serde_json::to_vec(&value).expect("serialize IDL JSON fixture");
    convert_idl(&bytes).expect("convert IDL fixture")
}

#[test]
fn current_idl_spec_passes_through() {
    let idl = convert_json(json!({
        "address": "11111111111111111111111111111111",
        "metadata": {
            "name": "current_program",
            "version": "0.1.0",
            "spec": "0.1.0"
        },
        "instructions": [
            {
                "name": "ping",
                "discriminator": [1, 2, 3, 4, 5, 6, 7, 8],
                "accounts": [],
                "args": []
            }
        ]
    }));

    assert_eq!(idl.address, "11111111111111111111111111111111");
    assert_eq!(idl.metadata.name, "current_program");
    assert_eq!(idl.metadata.spec, "0.1.0");
    assert_eq!(
        idl.instructions[0].discriminator,
        vec![1, 2, 3, 4, 5, 6, 7, 8]
    );
}

#[test]
fn legacy_idl_conversion_preserves_surface_and_generates_discriminators() {
    let idl = convert_json(json!({
        "version": "0.0.1",
        "name": "legacyCounter",
        "docs": ["legacy doc"],
        "instructions": [
            {
                "name": "setValue",
                "docs": ["set doc"],
                "accounts": [
                    {
                        "name": "counterAccount",
                        "isMut": true,
                        "isSigner": false,
                        "pda": {
                            "seeds": [
                                { "kind": "const", "type": "string", "value": "counter" },
                                { "kind": "arg", "type": "u64", "path": "newValue" },
                                {
                                    "kind": "account",
                                    "type": "publicKey",
                                    "path": "authority",
                                    "account": "AuthorityAccount"
                                }
                            ],
                            "programId": {
                                "kind": "account",
                                "type": "publicKey",
                                "path": "tokenProgram"
                            }
                        },
                        "relations": ["authority"]
                    },
                    {
                        "name": "authorityGroup",
                        "accounts": [
                            {
                                "name": "authority",
                                "isMut": false,
                                "isSigner": true,
                                "isOptional": true
                            }
                        ]
                    }
                ],
                "args": [{ "name": "newValue", "type": "u64" }],
                "returns": "u64"
            }
        ],
        "accounts": [
            {
                "name": "Counter",
                "type": {
                    "kind": "struct",
                    "fields": [{ "name": "value", "type": "u64" }]
                }
            }
        ],
        "types": [
            {
                "name": "Choice",
                "type": {
                    "kind": "enum",
                    "variants": [
                        { "name": "Empty" },
                        { "name": "Tuple", "fields": ["u8", { "defined": "Counter" }] }
                    ]
                }
            }
        ],
        "events": [
            {
                "name": "CounterEvent",
                "fields": [{ "name": "newValue", "type": "u64", "index": false }]
            }
        ],
        "errors": [{ "code": 6000, "name": "BadValue", "msg": "bad value" }],
        "constants": [{ "name": "MAX_VALUE", "type": "u64", "value": "10" }],
        "metadata": { "address": "11111111111111111111111111111111" }
    }));

    assert_eq!(idl.address, "11111111111111111111111111111111");
    assert_eq!(idl.metadata.name, "legacyCounter");
    assert_eq!(idl.metadata.version, "0.0.1");
    assert_eq!(idl.metadata.spec, IDL_SPEC);
    assert_eq!(idl.docs, vec!["legacy doc"]);

    let ix = &idl.instructions[0];
    assert_eq!(ix.name, "set_value");
    assert_eq!(ix.discriminator, discriminator("global", "set_value"));
    assert_eq!(ix.docs, vec!["set doc"]);
    assert_eq!(ix.args[0].name, "new_value");
    assert_eq!(ix.args[0].ty, IdlType::U64);
    assert_eq!(ix.returns, Some(IdlType::U64));

    let IdlInstructionAccountItem::Single(counter) = &ix.accounts[0] else {
        panic!("first legacy account should convert to a single account");
    };
    assert_eq!(counter.name, "counter_account");
    assert!(counter.writable);
    assert!(!counter.signer);
    assert_eq!(counter.relations, vec!["authority"]);

    let pda = counter.pda.as_ref().expect("legacy PDA should convert");
    assert_eq!(
        pda.seeds[0],
        IdlSeed::Const(anchor_lang_idl::types::IdlSeedConst {
            value: b"\"counter\"".to_vec()
        })
    );
    assert_eq!(
        pda.seeds[1],
        IdlSeed::Arg(anchor_lang_idl::types::IdlSeedArg {
            path: "new_value".into()
        })
    );
    assert_eq!(
        pda.seeds[2],
        IdlSeed::Account(anchor_lang_idl::types::IdlSeedAccount {
            path: "authority".into(),
            account: Some("AuthorityAccount".into())
        })
    );
    assert_eq!(
        pda.program,
        Some(IdlSeed::Account(anchor_lang_idl::types::IdlSeedAccount {
            path: "token_program".into(),
            account: None
        }))
    );

    let IdlInstructionAccountItem::Composite(group) = &ix.accounts[1] else {
        panic!("second legacy account should convert to a composite account");
    };
    assert_eq!(group.name, "authority_group");
    let IdlInstructionAccountItem::Single(authority) = &group.accounts[0] else {
        panic!("nested legacy account should convert to a single account");
    };
    assert!(authority.signer);
    assert!(authority.optional);

    assert_eq!(idl.accounts[0].name, "Counter");
    assert_eq!(
        idl.accounts[0].discriminator,
        discriminator("account", "Counter")
    );
    assert_eq!(idl.events[0].name, "CounterEvent");
    assert_eq!(
        idl.events[0].discriminator,
        discriminator("event", "CounterEvent")
    );
    assert_eq!(idl.errors[0].msg.as_deref(), Some("bad value"));
    assert_eq!(idl.constants[0].value, "10");

    let choice = idl
        .types
        .iter()
        .find(|ty| ty.name == "Choice")
        .expect("legacy type should convert");
    let IdlTypeDefTy::Enum { variants } = &choice.ty else {
        panic!("legacy enum should remain an enum");
    };
    assert_eq!(variants[1].name, "Tuple");
    assert!(matches!(
        variants[1].fields,
        Some(IdlDefinedFields::Tuple(_))
    ));
}

#[test]
fn legacy_idl_conversion_rejects_missing_program_address() {
    let bytes = serde_json::to_vec(&json!({
        "version": "0.0.1",
        "name": "legacyCounter",
        "instructions": [],
        "metadata": {}
    }))
    .expect("serialize IDL JSON fixture");

    let error = convert_idl(&bytes).expect_err("legacy IDL without metadata.address must fail");
    assert!(
        error
            .to_string()
            .contains("Program id missing in `idl.metadata.address` field"),
        "expected missing program id error, got: {error}"
    );
}

#[test]
fn unsupported_current_idl_spec_rejects() {
    let bytes = serde_json::to_vec(&json!({
        "address": "11111111111111111111111111111111",
        "metadata": {
            "name": "future_program",
            "version": "0.1.0",
            "spec": "9.9.9"
        },
        "instructions": []
    }))
    .expect("serialize IDL JSON fixture");

    let error = convert_idl(&bytes).expect_err("unsupported current IDL spec must fail");
    assert!(
        error
            .to_string()
            .contains("IDL spec not supported: `9.9.9`"),
        "expected unsupported spec error, got: {error}"
    );
}
