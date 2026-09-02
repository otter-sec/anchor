use anchor_lang::prelude::*;

declare_program!(external);

#[test]
pub fn test_account_parser() {
    use external::parsers::Account;

    // Empty
    assert!(Account::parse(&[]).is_err());

    // Correct discriminator but invalid data
    const DISC: &[u8] = external::accounts::MyAccount::DISCRIMINATOR;
    assert!(Account::parse(DISC).is_err());

    // Correct discriminator and valid data
    match Account::parse(&[DISC, &[1, 0, 0, 0]].concat()) {
        Ok(Account::MyAccount(my_account)) => assert_eq!(my_account.field, 1),
        Ok(_) => panic!("Expected MyAccount account variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    }

    // Unsafe zero-copy account
    let packed_account = external::accounts::PackedAccount {
        a: [1; 8],
        b: [2; 8],
    };
    const PACKED_DISC: &[u8] = external::accounts::PackedAccount::DISCRIMINATOR;
    match Account::parse(
        &[
            PACKED_DISC,
            anchor_lang::__private::bytemuck::bytes_of(&packed_account),
        ]
        .concat(),
    ) {
        Ok(Account::PackedAccount(packed_account)) => {
            let a = packed_account.a;
            let b = packed_account.b;
            assert_eq!(a, [1; 8]);
            assert_eq!(b, [2; 8]);
        }
        Ok(_) => panic!("Expected PackedAccount account variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    }
}

#[test]
pub fn test_event_parser() {
    use external::parsers::Event;

    // Empty
    assert!(Event::parse(&[]).is_err());

    // Correct discriminator but invalid data
    const DISC: &[u8] = external::events::MyEvent::DISCRIMINATOR;
    assert!(Event::parse(DISC).is_err());

    // Correct discriminator and valid data
    match Event::parse(&[DISC, &[1, 0, 0, 0]].concat()) {
        Ok(Event::MyEvent(my_event)) => assert_eq!(my_event.value, 1),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    }
}

#[test]
pub fn test_instruction_parser() {
    use {
        anchor_lang::solana_program::instruction::Instruction as SolanaInstruction,
        external::parsers::Instruction,
    };

    // Incorrect program
    assert!(Instruction::parse(&SolanaInstruction::new_with_bytes(
        system_program::ID,
        &[],
        vec![]
    ),)
    .is_err());
    // Incorrect instruction
    assert!(Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[],
        vec![]
    ),)
    .is_err());
    // Not enough accounts
    assert!(Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        external::client::args::Init::DISCRIMINATOR,
        vec![],
    ))
    .is_err());
    // Incorrect account(s)
    assert!(Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        external::client::args::Init::DISCRIMINATOR,
        vec![
            AccountMeta::default(),
            AccountMeta::default(),
            AccountMeta::default(),
        ],
    ),)
    .is_err());

    // Correct (`init`)
    let authority = Pubkey::from_str_const("Authority1111111111111111111111111111111111");
    let my_account = Pubkey::from_str_const("MyAccount1111111111111111111111111111111111");
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        external::client::args::Init::DISCRIMINATOR,
        vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(my_account, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    )) {
        Ok(Instruction::Init { accounts, args: _ }) => {
            assert_eq!(accounts.authority, authority);
            assert_eq!(accounts.my_account, my_account);
            assert_eq!(accounts.system_program, system_program::ID);
        }
        Ok(_) => panic!("Expected Init instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };
    // Missing arg
    assert!(Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        external::client::args::Update::DISCRIMINATOR,
        vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
        ],
    ),)
    .is_err());
    // Correct (`update`)
    let expected_args = external::client::args::Update { value: 1 };
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[
            external::client::args::Update::DISCRIMINATOR,
            &ser(&expected_args),
        ]
        .concat(),
        vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
        ],
    )) {
        Ok(Instruction::Update { accounts, args }) => {
            assert_eq!(accounts.authority, authority);
            assert_eq!(accounts.my_account, my_account);
            assert_eq!(args.value, expected_args.value);
        }
        Ok(_) => panic!("Expected Update instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };

    // Correct (`update_composite`)
    let expected_args = external::client::args::UpdateComposite { value: 2 };
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[
            external::client::args::UpdateComposite::DISCRIMINATOR,
            &ser(&expected_args),
        ]
        .concat(),
        vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
        ],
    )) {
        Ok(Instruction::UpdateComposite { accounts, args }) => {
            assert_eq!(accounts.update.authority, authority);
            assert_eq!(accounts.update.my_account, my_account);
            assert_eq!(args.value, expected_args.value);
        }
        Ok(_) => panic!("Expected UpdateComposite instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };

    // Correct (`update_non_instruction_composite`)
    let expected_args = external::client::args::UpdateNonInstructionComposite { value: 3 };
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[
            external::client::args::UpdateNonInstructionComposite::DISCRIMINATOR,
            &ser(&expected_args),
        ]
        .concat(),
        vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
            AccountMeta::new_readonly(external::ID, false),
        ],
    )) {
        Ok(Instruction::UpdateNonInstructionComposite { accounts, args }) => {
            assert_eq!(accounts.non_instruction_update.authority, authority);
            assert_eq!(accounts.non_instruction_update.my_account, my_account);
            assert_eq!(accounts.non_instruction_update.program, external::ID);
            assert_eq!(args.value, expected_args.value);
        }
        Ok(_) => panic!("Expected UpdateNonInstructionComposite instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };

    // Correct (`update_non_instruction_composite2`)
    let expected_args = external::client::args::UpdateNonInstructionComposite2 { value: 4 };
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[
            external::client::args::UpdateNonInstructionComposite2::DISCRIMINATOR,
            &ser(&expected_args),
        ]
        .concat(),
        vec![
            AccountMeta::new_readonly(external::ID, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
            AccountMeta::new_readonly(external::ID, false),
        ],
    )) {
        Ok(Instruction::UpdateNonInstructionComposite2 { accounts, args }) => {
            assert_eq!(accounts.non_instruction_update.program, external::ID);
            assert_eq!(
                accounts
                    .non_instruction_update_with_different_ident
                    .authority,
                authority
            );
            assert_eq!(
                accounts
                    .non_instruction_update_with_different_ident
                    .my_account,
                my_account
            );
            assert_eq!(
                accounts.non_instruction_update_with_different_ident.program,
                external::ID
            );
            assert_eq!(args.value, expected_args.value);
        }
        Ok(_) => panic!("Expected UpdateNonInstructionComposite2 instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };

    fn ser(val: impl AnchorSerialize) -> Vec<u8> {
        let mut w = vec![];
        val.serialize(&mut w).unwrap();
        w
    }

    // Test optional account parsing
    let authority = Pubkey::from_str_const("Authority1111111111111111111111111111111111");
    let my_account = Pubkey::from_str_const("MyAccount1111111111111111111111111111111111");
    let optional_account = Pubkey::from_str_const("optiona1Acc11111111111111111111111111111111");
    let expected_args = external::client::args::UpdateWithOptional { value: 5 };

    // Test with optional account present (Some)
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[
            external::client::args::UpdateWithOptional::DISCRIMINATOR,
            &ser(&expected_args),
        ]
        .concat(),
        vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
            AccountMeta::new(optional_account, false),
        ],
    )) {
        Ok(Instruction::UpdateWithOptional { accounts, args }) => {
            assert_eq!(accounts.authority, authority);
            assert_eq!(accounts.my_account, my_account);
            assert_eq!(accounts.optional_account, Some(optional_account));
            assert_eq!(args.value, expected_args.value);
        }
        Ok(_) => panic!("Expected UpdateWithOptional instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };

    // Test with optional account missing (None - using program ID as placeholder)
    match Instruction::parse(&SolanaInstruction::new_with_bytes(
        external::ID,
        &[
            external::client::args::UpdateWithOptional::DISCRIMINATOR,
            &ser(&expected_args),
        ]
        .concat(),
        vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(my_account, false),
            AccountMeta::new(external::ID, false), // Program ID as placeholder for missing optional
        ],
    )) {
        Ok(Instruction::UpdateWithOptional { accounts, args }) => {
            assert_eq!(accounts.authority, authority);
            assert_eq!(accounts.my_account, my_account);
            assert_eq!(accounts.optional_account, None);
            assert_eq!(args.value, expected_args.value);
        }
        Ok(_) => panic!("Expected UpdateWithOptional instruction variant"),
        Err(e) => panic!("Expected Ok result, got error: {:?}", e),
    };
}
