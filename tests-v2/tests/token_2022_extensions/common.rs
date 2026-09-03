use {
    anchor_lang::solana_program::instruction::AccountMeta,
    litesvm::{types::TransactionMetadata, LiteSVM},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::{
        extension::{set_account_type, ExtensionType, StateWithExtensionsMut},
        state::{Account as Token2022Account, AccountState, Mint},
    },
    std::path::PathBuf,
    tests_v2::{build_program, keypair_for, send_instruction},
};

pub const TOKEN_2022_MAINNET_SO: &str = "fixtures/programs/token_2022_mainnet.so";
pub const TOKEN_2022_MAINNET_SHA256: &str =
    "b2a7ce1ea6dfbcbc5ccb0e7f48f7c61dced1a86582d1c7d2e059ac54ed612da4";
pub const PROGRAMDATA_METADATA_LEN: usize = 45;

pub use {
    anchor_lang::solana_program::instruction::AccountMeta as Meta,
    litesvm, sha2, solana_loader_v3_interface, solana_rpc_client, spl_token_2022_interface,
    spl_token_2022_interface::extension::{BaseStateWithExtensions, BaseStateWithExtensionsMut},
};

pub fn token_2022_program_id() -> Pubkey {
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        .parse()
        .unwrap()
}

pub fn wrong_token_program_id() -> Pubkey {
    "7TdHZyhueZP4B8fvbgvbGPTH4bijkBPtpWc3wBfTmWQv"
        .parse()
        .unwrap()
}

pub fn program_id(address: &str) -> Pubkey {
    address.parse().unwrap()
}

pub fn token_2022_mainnet_so_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TOKEN_2022_MAINNET_SO)
}

pub fn setup(program_dir: &str, program_so: &str, address: &str) -> (LiteSVM, Keypair, Pubkey) {
    let test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir
            .join("programs/token-2022-extensions")
            .join(program_dir)
            .to_str()
            .unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let id = program_id(address);
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(id, deploy_dir.join(program_so))
        .expect("load extension fixture program");
    svm.add_program_from_file(token_2022_program_id(), token_2022_mainnet_so_path())
        .expect("load vendored mainnet Token-2022 program");

    let payer = keypair_for(&format!("token-2022-ext-{program_dir}-payer"));
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer, id)
}

pub fn seed_token_2022_account(svm: &mut LiteSVM, address: Pubkey, data: Vec<u8>) {
    svm.set_account(
        address,
        Account {
            lamports: 10_000_000,
            data,
            owner: token_2022_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed token-2022-owned account");
}

pub fn seed_mint_with_extensions(svm: &mut LiteSVM, address: Pubkey, extensions: &[ExtensionType]) {
    let len = ExtensionType::try_calculate_account_len::<Mint>(extensions)
        .expect("calculate token-2022 mint account length");
    seed_token_2022_account(svm, address, vec![0; len]);
}

pub fn seed_token_account_with_extensions(
    svm: &mut LiteSVM,
    address: Pubkey,
    extensions: &[ExtensionType],
) {
    let len = ExtensionType::try_calculate_account_len::<Token2022Account>(extensions)
        .expect("calculate token-2022 token account length");
    seed_token_2022_account(svm, address, vec![0; len]);
}

pub fn mark_mint_initialized(
    svm: &mut LiteSVM,
    address: Pubkey,
    mint_authority: Pubkey,
    freeze_authority: Option<Pubkey>,
) {
    let mut account = svm.get_account(&address).expect("mint exists");
    Mint {
        mint_authority: COption::Some(mint_authority),
        supply: 0,
        decimals: 6,
        is_initialized: true,
        freeze_authority: freeze_authority.map_or(COption::None, COption::Some),
    }
    .pack_into_slice(&mut account.data[..Mint::LEN]);
    set_account_type::<Mint>(&mut account.data).expect("set mint account type");
    svm.set_account(address, account)
        .expect("write initialized mint base state");
}

pub fn initialized_mint_data_with_space(
    mint_authority: Pubkey,
    freeze_authority: Option<Pubkey>,
    extensions: &[ExtensionType],
    extra_space: usize,
) -> Vec<u8> {
    let len = ExtensionType::try_calculate_account_len::<Mint>(extensions)
        .expect("calculate initialized token-2022 mint account length")
        + extra_space;
    let mut data = vec![0; len];
    Mint {
        mint_authority: COption::Some(mint_authority),
        supply: 0,
        decimals: 6,
        is_initialized: true,
        freeze_authority: freeze_authority.map_or(COption::None, COption::Some),
    }
    .pack_into_slice(&mut data[..Mint::LEN]);
    set_account_type::<Mint>(&mut data).expect("set mint account type");
    data
}

pub fn mint_state(data: &mut [u8]) -> StateWithExtensionsMut<'_, Mint> {
    const MINT_INITIALIZED_OFFSET: usize = 36 + 8 + 1;
    if data.get(MINT_INITIALIZED_OFFSET) == Some(&1) {
        StateWithExtensionsMut::<Mint>::unpack(data).expect("unpack initialized mint")
    } else {
        StateWithExtensionsMut::<Mint>::unpack_uninitialized(data)
            .expect("unpack uninitialized mint")
    }
}

pub fn initialized_token_account_data(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    extensions: &[ExtensionType],
) -> Vec<u8> {
    let len = ExtensionType::try_calculate_account_len::<Token2022Account>(extensions)
        .expect("calculate initialized token-2022 account length");
    let mut data = vec![0; len];
    Token2022Account {
        mint,
        owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    }
    .pack_into_slice(&mut data[..Token2022Account::LEN]);
    set_account_type::<Token2022Account>(&mut data).expect("set token account type");
    data
}

pub fn seed_initialized_memo_transfer_account(
    svm: &mut LiteSVM,
    address: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
) {
    use spl_token_2022_interface::extension::memo_transfer::MemoTransfer;

    let mut data = initialized_token_account_data(mint, owner, 0, &[ExtensionType::MemoTransfer]);
    {
        let mut state =
            StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).expect("unpack token");
        state
            .init_extension::<MemoTransfer>(false)
            .expect("initialize memo-transfer extension slot");
    }
    seed_token_2022_account(svm, address, data);
}

pub fn address_bytes(address: Pubkey) -> Vec<u8> {
    address.to_bytes().to_vec()
}

pub fn assert_token_2022_cpi_succeeded(metadata: &TransactionMetadata, context: &str) {
    let token_id = token_2022_program_id().to_string();
    assert!(
        metadata
            .logs
            .iter()
            .any(|log| log.contains(&format!("Program {token_id} invoke"))),
        "{context}: Token-2022 program was not invoked\n{}",
        metadata.pretty_logs()
    );
    assert!(
        metadata
            .logs
            .iter()
            .any(|log| log.contains(&format!("Program {token_id} success"))),
        "{context}: Token-2022 program did not report success\n{}",
        metadata.pretty_logs()
    );
}

pub fn expect_incorrect_program<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("IncorrectProgramId") || error.contains("incorrect program id"),
        "{context}: expected IncorrectProgramId, got:\n{error}"
    );
}

pub fn expect_program_failure<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("ProgramFailedToComplete")
            || error.contains("program failed")
            || error.contains("panicked"),
        "{context}: expected program failure, got:\n{error}"
    );
}

pub fn send(
    svm: &mut LiteSVM,
    id: Pubkey,
    data: Vec<u8>,
    metas: Vec<AccountMeta>,
    payer: &Keypair,
    signers: &[&Keypair],
) -> anyhow::Result<TransactionMetadata> {
    send_instruction(svm, id, data, metas, payer, signers)
}
