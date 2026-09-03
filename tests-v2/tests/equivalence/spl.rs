use {
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    litesvm::LiteSVM,
    proptest::prelude::*,
    sha2::{Digest, Sha256},
    solana_account::Account,
    solana_message::{Message, VersionedMessage},
    solana_program_option::COption as Token2022COption,
    solana_program_pack::Pack as Token2022Pack,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_token::{
        solana_program::{
            program_option::COption, program_pack::Pack, pubkey::Pubkey as SplPubkey,
        },
        state::{Account as SplTokenAccount, AccountState as SplAccountState, Mint as SplMint},
    },
    spl_token_2022_interface::{
        extension::{
            confidential_mint_burn::ConfidentialMintBurn,
            confidential_transfer::{ConfidentialTransferAccount, ConfidentialTransferMint},
            confidential_transfer_fee::{
                ConfidentialTransferFeeAmount, ConfidentialTransferFeeConfig,
            },
            cpi_guard::CpiGuard,
            default_account_state::DefaultAccountState,
            group_member_pointer::GroupMemberPointer,
            group_pointer::GroupPointer,
            immutable_owner::ImmutableOwner,
            interest_bearing_mint::InterestBearingConfig,
            memo_transfer::MemoTransfer,
            metadata_pointer::MetadataPointer,
            mint_close_authority::MintCloseAuthority,
            non_transferable::{NonTransferable, NonTransferableAccount},
            pausable::{PausableAccount, PausableConfig},
            permanent_delegate::PermanentDelegate,
            scaled_ui_amount::ScaledUiAmountConfig,
            transfer_fee::TransferFeeAmount,
            transfer_fee::TransferFeeConfig,
            transfer_hook::{TransferHook, TransferHookAccount},
            BaseStateWithExtensionsMut, ExtensionType as Token2022ExtensionType,
            StateWithExtensionsMut,
        },
        state::{
            Account as Token2022Account, AccountState as Token2022AccountState,
            Mint as Token2022Mint,
        },
    },
    spl_token_group_interface::state::{TokenGroup, TokenGroupMember},
    std::sync::OnceLock,
    tests_v2::{build_program, keypair_for},
};

const OBSERVATION_LEN: usize = 256;
const MAX_SMALL_RANDOM_DATA_LEN: usize = 512;
const MAX_LARGE_RANDOM_DATA_LEN: usize = 20 * 1024;
const DEFAULT_ACCOUNT_LAMPORTS: u64 = 10_000_000;
const DEFAULT_PAYER_LAMPORTS: u64 = 10_000_000_000;
const V2_DISC_STRICT_MINT: u8 = 0;
const V2_DISC_STRICT_TOKEN_ACCOUNT: u8 = 1;
const V2_DISC_INTERFACE_MINT: u8 = 2;
const V2_DISC_INTERFACE_TOKEN_ACCOUNT: u8 = 3;
const V2_DISC_INTERFACE_MINT_EXTENSION: u8 = 4;
const V2_DISC_INTERFACE_TOKEN_ACCOUNT_EXTENSION: u8 = 5;
const TAG_INTERFACE_MINT_EXTENSION: u8 = 5;
const TAG_INTERFACE_TOKEN_EXTENSION: u8 = 6;
const EXTENSION_STATUS_FOUND: u8 = 1;
const EXTENSION_STATUS_ILLEGAL_OWNER: u8 = 2;
const EXTENSION_STATUS_ACCESS_ERROR: u8 = 3;

#[derive(Clone, Copy, Debug)]
enum OracleVersion {
    V1,
    V2,
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    StrictMint,
    StrictTokenAccount,
    InterfaceMint,
    InterfaceTokenAccount,
    InterfaceMintExtension(MintExtensionObservation),
    InterfaceTokenAccountExtension(TokenAccountExtensionObservation),
}

impl Operation {
    fn v1_name(self) -> &'static str {
        match self {
            Operation::StrictMint => "check_strict_mint",
            Operation::StrictTokenAccount => "check_strict_token_account",
            Operation::InterfaceMint => "check_interface_mint",
            Operation::InterfaceTokenAccount => "check_interface_token_account",
            Operation::InterfaceMintExtension(_) => "check_interface_mint_extension",
            Operation::InterfaceTokenAccountExtension(_) => {
                "check_interface_token_account_extension"
            }
        }
    }

    fn v2_discriminator(self) -> u8 {
        match self {
            Operation::StrictMint => V2_DISC_STRICT_MINT,
            Operation::StrictTokenAccount => V2_DISC_STRICT_TOKEN_ACCOUNT,
            Operation::InterfaceMint => V2_DISC_INTERFACE_MINT,
            Operation::InterfaceTokenAccount => V2_DISC_INTERFACE_TOKEN_ACCOUNT,
            Operation::InterfaceMintExtension(_) => V2_DISC_INTERFACE_MINT_EXTENSION,
            Operation::InterfaceTokenAccountExtension(_) => {
                V2_DISC_INTERFACE_TOKEN_ACCOUNT_EXTENSION
            }
        }
    }

    fn argument(self) -> Option<u8> {
        match self {
            Operation::InterfaceMintExtension(observation) => Some(observation as u8),
            Operation::InterfaceTokenAccountExtension(observation) => Some(observation as u8),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum MintExtensionObservation {
    MetadataPointer = 0,
    GroupPointer = 1,
    GroupMemberPointer = 2,
    TransferHook = 3,
    MintCloseAuthority = 4,
    PermanentDelegate = 5,
    TransferFeeConfig = 6,
    PausableConfig = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum TokenAccountExtensionObservation {
    TransferFeeAmount = 0,
    CpiGuard = 1,
    TransferHookAccount = 2,
    PausableAccount = 3,
}

#[derive(Clone, Debug)]
enum MintExtensionCase {
    MetadataPointer(Vec<u8>),
    GroupPointer(Vec<u8>),
    GroupMemberPointer(Vec<u8>),
    TransferHook(Vec<u8>),
    MintCloseAuthority(Vec<u8>),
    PermanentDelegate(Vec<u8>),
    TransferFeeConfig(Vec<u8>),
    PausableConfig(Vec<u8>),
}

#[derive(Clone, Debug)]
enum TokenAccountExtensionCase {
    TransferFeeAmount(Vec<u8>),
    CpiGuard(Vec<u8>),
    TransferHookAccount(Vec<u8>),
    PausableAccount(Vec<u8>),
}

#[derive(Clone, Debug)]
struct Case {
    operation: Operation,
    payer_lamports: u64,
    target: AccountCase,
    output: OutputAccountCase,
    target_signer: bool,
    target_writable: bool,
    output_signer: bool,
    output_writable: bool,
}

#[derive(Clone, Debug)]
struct AccountCase {
    lamports: u64,
    data: Vec<u8>,
    owner: Pubkey,
    executable: bool,
    rent_epoch: u64,
}

#[derive(Clone, Debug)]
struct OutputAccountCase {
    lamports: u64,
    data: Vec<u8>,
    owner: OutputOwner,
    executable: bool,
    rent_epoch: u64,
}

#[derive(Clone, Copy, Debug)]
enum OutputOwner {
    Oracle,
    Address(Pubkey),
}

impl OutputOwner {
    fn resolve(self, program_id: Pubkey) -> Pubkey {
        match self {
            OutputOwner::Oracle => program_id,
            OutputOwner::Address(owner) => owner,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NormalizedResult {
    Ok(Vec<NormalizedAccount>),
    Err,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedAccount {
    key: Pubkey,
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
}

fn v1_program_id() -> Pubkey {
    "HYLtHw8VKojJTZXzodkeaerYjK2um5bSrqyGwMYYTjNL"
        .parse()
        .unwrap()
}

fn v2_program_id() -> Pubkey {
    "5FGXfwXAgDDy76hUXWQEYdBF8ztPezhq7AwibdDtFWvs"
        .parse()
        .unwrap()
}

fn oracle_owner_sentinel() -> Pubkey {
    "EQSJ8wDNihTnzyBwu73sUNdo8fF5ZrsPkPHr2m9KpTMs"
        .parse()
        .unwrap()
}

fn token_program_id() -> Pubkey {
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        .parse()
        .unwrap()
}

fn token_2022_program_id() -> Pubkey {
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        .parse()
        .unwrap()
}

fn setup() -> std::path::PathBuf {
    static DEPLOY_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

    DEPLOY_DIR
        .get_or_init(|| {
            let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let deploy_dir = test_dir.join("target/deploy");
            build_program(
                test_dir
                    .join("programs/equivalence/spl/v1")
                    .to_str()
                    .unwrap(),
                deploy_dir.to_str().unwrap(),
            );
            build_program(
                test_dir
                    .join("programs/equivalence/spl/v2")
                    .to_str()
                    .unwrap(),
                deploy_dir.to_str().unwrap(),
            );
            deploy_dir
        })
        .clone()
}

fn run_tx(deploy_dir: &std::path::Path, version: OracleVersion, case: &Case) -> NormalizedResult {
    let mut svm = LiteSVM::new();
    let payer = keypair_for("spl-equivalence-payer");
    if svm.airdrop(&payer.pubkey(), case.payer_lamports).is_err() {
        return NormalizedResult::Err;
    }

    let program_id = match version {
        OracleVersion::V1 => {
            let id = v1_program_id();
            svm.add_program_from_file(id, deploy_dir.join("equivalence_spl_v1.so"))
                .expect("load v1 oracle");
            id
        }
        OracleVersion::V2 => {
            let id = v2_program_id();
            svm.add_program_from_file(id, deploy_dir.join("equivalence_spl_v2.so"))
                .expect("load v2 oracle");
            id
        }
    };

    let target = keypair_for("spl-equivalence-target").pubkey();
    let out = keypair_for("spl-equivalence-output").pubkey();

    if svm
        .set_account(
            target,
            Account {
                lamports: case.target.lamports,
                data: case.target.data.clone(),
                owner: case.target.owner,
                executable: case.target.executable,
                rent_epoch: case.target.rent_epoch,
            },
        )
        .is_err()
    {
        return NormalizedResult::Err;
    }
    if svm
        .set_account(
            out,
            Account {
                lamports: case.output.lamports,
                data: case.output.data.clone(),
                owner: case.output.owner.resolve(program_id),
                executable: case.output.executable,
                rent_epoch: case.output.rent_epoch,
            },
        )
        .is_err()
    {
        return NormalizedResult::Err;
    }

    let instruction_data = match version {
        OracleVersion::V1 => v1_instruction_data(case.operation),
        OracleVersion::V2 => {
            let mut data = vec![case.operation.v2_discriminator()];
            if let Some(argument) = case.operation.argument() {
                data.push(argument);
            }
            data
        }
    };
    let instruction = Instruction::new_with_bytes(
        program_id,
        &instruction_data,
        vec![
            AccountMeta {
                pubkey: target,
                is_signer: case.target_signer,
                is_writable: case.target_writable,
            },
            AccountMeta {
                pubkey: out,
                is_signer: case.output_signer,
                is_writable: case.output_writable,
            },
        ],
    );
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let transaction =
        match VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&payer]) {
            Ok(transaction) => transaction,
            Err(_) => return NormalizedResult::Err,
        };

    match svm.send_transaction(transaction) {
        Ok(_) => NormalizedResult::Ok(snapshot_accounts(&svm, &[payer.pubkey(), target, out])),
        Err(_) => NormalizedResult::Err,
    }
}

fn snapshot_accounts(svm: &LiteSVM, keys: &[Pubkey]) -> Vec<NormalizedAccount> {
    keys.iter()
        .map(|key| {
            let account = svm.get_account(key).expect("snapshot account exists");
            NormalizedAccount {
                key: *key,
                lamports: account.lamports,
                owner: normalize_owner(account.owner),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                data: account.data,
            }
        })
        .collect()
}

fn normalize_owner(owner: Pubkey) -> Pubkey {
    if owner == v1_program_id() || owner == v2_program_id() {
        oracle_owner_sentinel()
    } else {
        owner
    }
}

fn v1_instruction_data(operation: Operation) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", operation.v1_name()).as_bytes());
    let mut data = hasher.finalize()[..8].to_vec();
    if let Some(argument) = operation.argument() {
        data.push(argument);
    }
    data
}

fn mint_case_strategy(
    operation: Operation,
    owner: impl Strategy<Value = Pubkey>,
) -> impl Strategy<Value = Case> {
    (
        Just(operation),
        payer_lamports_strategy(),
        account_case_strategy(owner, mint_data_strategy()),
        output_account_case_strategy(),
        signer_strategy(),
        writable_strategy(),
        signer_strategy(),
        writable_strategy(),
    )
        .prop_map(
            |(
                operation,
                payer_lamports,
                target,
                output,
                target_signer,
                target_writable,
                output_signer,
                output_writable,
            )| Case {
                operation,
                payer_lamports,
                target,
                output,
                target_signer,
                target_writable,
                output_signer,
                output_writable,
            },
        )
}

fn token_account_case_strategy(
    operation: Operation,
    owner: impl Strategy<Value = Pubkey>,
) -> impl Strategy<Value = Case> {
    (
        Just(operation),
        payer_lamports_strategy(),
        account_case_strategy(owner, token_account_data_strategy()),
        output_account_case_strategy(),
        signer_strategy(),
        writable_strategy(),
        signer_strategy(),
        writable_strategy(),
    )
        .prop_map(
            |(
                operation,
                payer_lamports,
                target,
                output,
                target_signer,
                target_writable,
                output_signer,
                output_writable,
            )| Case {
                operation,
                payer_lamports,
                target,
                output,
                target_signer,
                target_writable,
                output_signer,
                output_writable,
            },
        )
}

fn mint_extension_case_strategy() -> impl Strategy<Value = Case> {
    mint_extension_observation_strategy().prop_flat_map(|observation| {
        (
            Just(Operation::InterfaceMintExtension(observation)),
            payer_lamports_strategy(),
            account_case_strategy(
                interface_owner_strategy(),
                token_2022_mint_extension_data_strategy(observation),
            ),
            output_account_case_strategy(),
            signer_strategy(),
            writable_strategy(),
            signer_strategy(),
            writable_strategy(),
        )
            .prop_map(
                |(
                    operation,
                    payer_lamports,
                    target,
                    output,
                    target_signer,
                    target_writable,
                    output_signer,
                    output_writable,
                )| Case {
                    operation,
                    payer_lamports,
                    target,
                    output,
                    target_signer,
                    target_writable,
                    output_signer,
                    output_writable,
                },
            )
    })
}

fn token_account_extension_case_strategy() -> impl Strategy<Value = Case> {
    token_account_extension_observation_strategy().prop_flat_map(|observation| {
        (
            Just(Operation::InterfaceTokenAccountExtension(observation)),
            payer_lamports_strategy(),
            account_case_strategy(
                interface_owner_strategy(),
                token_2022_token_account_extension_data_strategy(observation),
            ),
            output_account_case_strategy(),
            signer_strategy(),
            writable_strategy(),
            signer_strategy(),
            writable_strategy(),
        )
            .prop_map(
                |(
                    operation,
                    payer_lamports,
                    target,
                    output,
                    target_signer,
                    target_writable,
                    output_signer,
                    output_writable,
                )| Case {
                    operation,
                    payer_lamports,
                    target,
                    output,
                    target_signer,
                    target_writable,
                    output_signer,
                    output_writable,
                },
            )
    })
}

fn account_case_strategy(
    owner: impl Strategy<Value = Pubkey>,
    data: impl Strategy<Value = Vec<u8>>,
) -> impl Strategy<Value = AccountCase> {
    (
        account_lamports_strategy(),
        data,
        owner,
        executable_strategy(),
        rent_epoch_strategy(),
    )
        .prop_map(
            |(lamports, data, owner, executable, rent_epoch)| AccountCase {
                lamports,
                data,
                owner,
                executable,
                rent_epoch,
            },
        )
}

fn output_account_case_strategy() -> impl Strategy<Value = OutputAccountCase> {
    (
        account_lamports_strategy(),
        output_data_strategy(),
        output_owner_strategy(),
        executable_strategy(),
        rent_epoch_strategy(),
    )
        .prop_map(
            |(lamports, data, owner, executable, rent_epoch)| OutputAccountCase {
                lamports,
                data,
                owner,
                executable,
                rent_epoch,
            },
        )
}

fn payer_lamports_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        8 => Just(DEFAULT_PAYER_LAMPORTS),
        1 => 0u64..50_000,
        1 => 50_000u64..20_000_000_000,
    ]
}

fn account_lamports_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        6 => Just(DEFAULT_ACCOUNT_LAMPORTS),
        1 => Just(0),
        2 => 1u64..1_000_000_000_000,
    ]
}

fn output_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        6 => Just(vec![0; OBSERVATION_LEN]),
        4 => arbitrary_data_strategy(),
    ]
}

fn arbitrary_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        8 => prop::collection::vec(any::<u8>(), 0..MAX_SMALL_RANDOM_DATA_LEN),
        1 => prop::collection::vec(any::<u8>(), 0..MAX_LARGE_RANDOM_DATA_LEN),
    ]
}

fn output_owner_strategy() -> impl Strategy<Value = OutputOwner> {
    prop_oneof![
        6 => Just(OutputOwner::Oracle),
        1 => Just(OutputOwner::Address(token_program_id())),
        1 => Just(OutputOwner::Address(token_2022_program_id())),
        1 => any::<[u8; 32]>().prop_map(|owner| OutputOwner::Address(Pubkey::new_from_array(owner))),
    ]
}

fn executable_strategy() -> impl Strategy<Value = bool> {
    prop_oneof![
        8 => Just(false),
        1 => Just(true),
    ]
}

fn signer_strategy() -> impl Strategy<Value = bool> {
    prop_oneof![
        8 => Just(false),
        1 => Just(true),
    ]
}

fn writable_strategy() -> impl Strategy<Value = bool> {
    prop_oneof![
        8 => Just(true),
        1 => Just(false),
    ]
}

fn rent_epoch_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        6 => Just(0),
        1 => 1u64..1_000_000,
        1 => any::<u64>(),
    ]
}

fn strict_owner_strategy() -> impl Strategy<Value = Pubkey> {
    prop_oneof![
        6 => Just(token_program_id()),
        2 => Just(token_2022_program_id()),
        1 => any::<[u8; 32]>().prop_map(Pubkey::new_from_array),
    ]
}

fn interface_owner_strategy() -> impl Strategy<Value = Pubkey> {
    prop_oneof![
        4 => Just(token_program_id()),
        4 => Just(token_2022_program_id()),
        1 => any::<[u8; 32]>().prop_map(Pubkey::new_from_array),
    ]
}

fn mint_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        2 => arbitrary_data_strategy(),
        3 => prop::collection::vec(any::<u8>(), SplMint::LEN).prop_map(|mut data| {
            data[36 + 8 + 1] = 1;
            data
        }),
        2 => canonical_mint_data_strategy(),
        2 => token_2022_mint_data_strategy(),
        4 => malformed_mint_coption_data_strategy(),
    ]
}

fn token_account_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        2 => arbitrary_data_strategy(),
        3 => prop::collection::vec(any::<u8>(), SplTokenAccount::LEN).prop_map(|mut data| {
            data[32 + 32 + 8 + 4 + 32] = 1;
            data
        }),
        2 => canonical_token_account_data_strategy(),
        2 => token_2022_token_account_data_strategy(),
        4 => malformed_token_coption_data_strategy(),
    ]
}

fn canonical_mint_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    (
        any::<[u8; 32]>(),
        any::<u64>(),
        any::<u8>(),
        any::<bool>(),
        prop::option::of(any::<[u8; 32]>()),
    )
        .prop_map(
            |(authority, supply, decimals, has_freeze_authority, freeze_authority)| {
                let mut data = vec![0; SplMint::LEN];
                let mint = SplMint {
                    mint_authority: COption::Some(SplPubkey::new_from_array(authority)),
                    supply,
                    decimals,
                    is_initialized: true,
                    freeze_authority: if has_freeze_authority {
                        COption::Some(SplPubkey::new_from_array(
                            freeze_authority.unwrap_or([7; 32]),
                        ))
                    } else {
                        COption::None
                    },
                };
                SplMint::pack(mint, &mut data).unwrap();
                data
            },
        )
}

fn token_2022_mint_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        8 => (
            token_2022_mint_base_strategy(),
            prop::collection::vec(token_2022_mint_extension_strategy(), 1..6),
        )
            .prop_map(|(mint, extension_types)| token_2022_mint_data(mint, &extension_types)),
        1 => (
            token_2022_mint_base_strategy(),
            prop::collection::vec(any::<u8>(), 0..128),
        )
            .prop_map(|(mint, metadata)| token_2022_mint_token_metadata_data(mint, &metadata)),
    ]
}

fn canonical_token_account_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    (
        any::<[u8; 32]>(),
        any::<[u8; 32]>(),
        any::<u64>(),
        prop::option::of(any::<[u8; 32]>()),
        prop_oneof![
            Just(SplAccountState::Initialized),
            Just(SplAccountState::Frozen),
        ],
        prop::option::of(any::<u64>()),
        any::<u64>(),
        prop::option::of(any::<[u8; 32]>()),
    )
        .prop_map(
            |(
                mint,
                owner,
                amount,
                delegate,
                state,
                is_native,
                delegated_amount,
                close_authority,
            )| {
                let mut data = vec![0; SplTokenAccount::LEN];
                let account = SplTokenAccount {
                    mint: SplPubkey::new_from_array(mint),
                    owner: SplPubkey::new_from_array(owner),
                    amount,
                    delegate: delegate
                        .map(SplPubkey::new_from_array)
                        .map(COption::Some)
                        .unwrap_or(COption::None),
                    state,
                    is_native: is_native.map(COption::Some).unwrap_or(COption::None),
                    delegated_amount,
                    close_authority: close_authority
                        .map(SplPubkey::new_from_array)
                        .map(COption::Some)
                        .unwrap_or(COption::None),
                };
                SplTokenAccount::pack(account, &mut data).unwrap();
                data
            },
        )
}

fn token_2022_token_account_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    (
        token_2022_account_base_strategy(),
        prop::collection::vec(token_2022_account_extension_strategy(), 1..6),
    )
        .prop_map(|(account, extension_types)| {
            token_2022_token_account_data(account, &extension_types)
        })
}

fn token_2022_mint_base_strategy() -> impl Strategy<Value = Token2022Mint> {
    (
        prop::option::of(any::<[u8; 32]>()),
        any::<u64>(),
        any::<u8>(),
        prop::option::of(any::<[u8; 32]>()),
    )
        .prop_map(
            |(mint_authority, supply, decimals, freeze_authority)| Token2022Mint {
                mint_authority: mint_authority
                    .map(Pubkey::new_from_array)
                    .map(Token2022COption::Some)
                    .unwrap_or(Token2022COption::None),
                supply,
                decimals,
                is_initialized: true,
                freeze_authority: freeze_authority
                    .map(Pubkey::new_from_array)
                    .map(Token2022COption::Some)
                    .unwrap_or(Token2022COption::None),
            },
        )
}

fn token_2022_account_base_strategy() -> impl Strategy<Value = Token2022Account> {
    (
        any::<[u8; 32]>(),
        any::<[u8; 32]>(),
        any::<u64>(),
        prop::option::of(any::<[u8; 32]>()),
        prop_oneof![
            Just(Token2022AccountState::Initialized),
            Just(Token2022AccountState::Frozen),
        ],
        prop::option::of(any::<u64>()),
        any::<u64>(),
        prop::option::of(any::<[u8; 32]>()),
    )
        .prop_map(
            |(
                mint,
                owner,
                amount,
                delegate,
                state,
                is_native,
                delegated_amount,
                close_authority,
            )| Token2022Account {
                mint: Pubkey::new_from_array(mint),
                owner: Pubkey::new_from_array(owner),
                amount,
                delegate: delegate
                    .map(Pubkey::new_from_array)
                    .map(Token2022COption::Some)
                    .unwrap_or(Token2022COption::None),
                state,
                is_native: is_native
                    .map(Token2022COption::Some)
                    .unwrap_or(Token2022COption::None),
                delegated_amount,
                close_authority: close_authority
                    .map(Pubkey::new_from_array)
                    .map(Token2022COption::Some)
                    .unwrap_or(Token2022COption::None),
            },
        )
}

fn token_2022_mint_extension_strategy() -> impl Strategy<Value = Token2022ExtensionType> {
    prop_oneof![
        Just(Token2022ExtensionType::TransferFeeConfig),
        Just(Token2022ExtensionType::MintCloseAuthority),
        Just(Token2022ExtensionType::ConfidentialTransferMint),
        Just(Token2022ExtensionType::DefaultAccountState),
        Just(Token2022ExtensionType::NonTransferable),
        Just(Token2022ExtensionType::InterestBearingConfig),
        Just(Token2022ExtensionType::PermanentDelegate),
        Just(Token2022ExtensionType::TransferHook),
        Just(Token2022ExtensionType::ConfidentialTransferFeeConfig),
        Just(Token2022ExtensionType::MetadataPointer),
        Just(Token2022ExtensionType::GroupPointer),
        Just(Token2022ExtensionType::TokenGroup),
        Just(Token2022ExtensionType::GroupMemberPointer),
        Just(Token2022ExtensionType::TokenGroupMember),
        Just(Token2022ExtensionType::ConfidentialMintBurn),
        Just(Token2022ExtensionType::ScaledUiAmount),
        Just(Token2022ExtensionType::Pausable),
    ]
}

fn token_2022_account_extension_strategy() -> impl Strategy<Value = Token2022ExtensionType> {
    prop_oneof![
        Just(Token2022ExtensionType::TransferFeeAmount),
        Just(Token2022ExtensionType::ConfidentialTransferAccount),
        Just(Token2022ExtensionType::ImmutableOwner),
        Just(Token2022ExtensionType::MemoTransfer),
        Just(Token2022ExtensionType::CpiGuard),
        Just(Token2022ExtensionType::NonTransferableAccount),
        Just(Token2022ExtensionType::TransferHookAccount),
        Just(Token2022ExtensionType::ConfidentialTransferFeeAmount),
        Just(Token2022ExtensionType::PausableAccount),
    ]
}

fn mint_extension_observation_strategy() -> impl Strategy<Value = MintExtensionObservation> {
    prop_oneof![
        Just(MintExtensionObservation::MetadataPointer),
        Just(MintExtensionObservation::GroupPointer),
        Just(MintExtensionObservation::GroupMemberPointer),
        Just(MintExtensionObservation::TransferHook),
        Just(MintExtensionObservation::MintCloseAuthority),
        Just(MintExtensionObservation::PermanentDelegate),
        Just(MintExtensionObservation::TransferFeeConfig),
        Just(MintExtensionObservation::PausableConfig),
    ]
}

fn token_account_extension_observation_strategy(
) -> impl Strategy<Value = TokenAccountExtensionObservation> {
    prop_oneof![
        Just(TokenAccountExtensionObservation::TransferFeeAmount),
        Just(TokenAccountExtensionObservation::CpiGuard),
        Just(TokenAccountExtensionObservation::TransferHookAccount),
        Just(TokenAccountExtensionObservation::PausableAccount),
    ]
}

fn token_2022_mint_extension_data_strategy(
    observation: MintExtensionObservation,
) -> BoxedStrategy<Vec<u8>> {
    let present = token_2022_mint_present_extension_data_strategy(observation);

    let absent = (
        token_2022_mint_base_strategy(),
        mint_extension_case_for_observation_strategy(alternate_mint_extension_observation(
            observation,
        )),
    )
        .prop_map(|(mint, extension)| {
            token_2022_mint_data_with_extension_cases(mint, &[extension])
        });

    let corrupted = token_2022_mint_present_extension_data_strategy(observation)
        .prop_flat_map(|data| corrupt_data_strategy(data));

    prop_oneof![
        5 => present,
        3 => absent,
        2 => arbitrary_data_strategy(),
        2 => corrupted,
    ]
    .boxed()
}

fn token_2022_mint_present_extension_data_strategy(
    observation: MintExtensionObservation,
) -> impl Strategy<Value = Vec<u8>> {
    (
        token_2022_mint_base_strategy(),
        mint_extension_case_for_observation_strategy(observation),
        prop::bool::ANY,
    )
        .prop_map(move |(mint, observed_extension, include_extra)| {
            let mut extensions = vec![observed_extension];
            if include_extra {
                extensions.push(mint_extension_case_for_observation(
                    alternate_mint_extension_observation(observation),
                    vec![0; 128],
                ));
            }
            token_2022_mint_data_with_extension_cases(mint, &extensions)
        })
}

fn token_2022_token_account_extension_data_strategy(
    observation: TokenAccountExtensionObservation,
) -> BoxedStrategy<Vec<u8>> {
    let present = token_2022_token_account_present_extension_data_strategy(observation);

    let absent = (
        token_2022_account_base_strategy(),
        token_account_extension_case_for_observation_strategy(
            alternate_token_account_extension_observation(observation),
        ),
    )
        .prop_map(|(account, extension)| {
            token_2022_token_account_data_with_extension_cases(account, &[extension])
        });

    let corrupted = token_2022_token_account_present_extension_data_strategy(observation)
        .prop_flat_map(|data| corrupt_data_strategy(data));

    prop_oneof![
        5 => present,
        3 => absent,
        2 => arbitrary_data_strategy(),
        2 => corrupted,
    ]
    .boxed()
}

fn token_2022_token_account_present_extension_data_strategy(
    observation: TokenAccountExtensionObservation,
) -> impl Strategy<Value = Vec<u8>> {
    (
        token_2022_account_base_strategy(),
        token_account_extension_case_for_observation_strategy(observation),
        prop::bool::ANY,
    )
        .prop_map(move |(account, observed_extension, include_extra)| {
            let mut extensions = vec![observed_extension];
            if include_extra {
                extensions.push(token_account_extension_case_for_observation(
                    alternate_token_account_extension_observation(observation),
                    vec![0; 128],
                ));
            }
            token_2022_token_account_data_with_extension_cases(account, &extensions)
        })
}

fn extension_payload_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        2 => Just(vec![0; 128]),
        1 => Just(Vec::new()),
        7 => prop::collection::vec(any::<u8>(), 0..128),
    ]
}

fn mint_extension_case_for_observation_strategy(
    observation: MintExtensionObservation,
) -> impl Strategy<Value = MintExtensionCase> {
    extension_payload_strategy()
        .prop_map(move |payload| mint_extension_case_for_observation(observation, payload))
}

fn token_account_extension_case_for_observation_strategy(
    observation: TokenAccountExtensionObservation,
) -> impl Strategy<Value = TokenAccountExtensionCase> {
    extension_payload_strategy()
        .prop_map(move |payload| token_account_extension_case_for_observation(observation, payload))
}

fn mint_extension_case_for_observation(
    observation: MintExtensionObservation,
    payload: Vec<u8>,
) -> MintExtensionCase {
    match observation {
        MintExtensionObservation::MetadataPointer => MintExtensionCase::MetadataPointer(payload),
        MintExtensionObservation::GroupPointer => MintExtensionCase::GroupPointer(payload),
        MintExtensionObservation::GroupMemberPointer => {
            MintExtensionCase::GroupMemberPointer(payload)
        }
        MintExtensionObservation::TransferHook => MintExtensionCase::TransferHook(payload),
        MintExtensionObservation::MintCloseAuthority => {
            MintExtensionCase::MintCloseAuthority(payload)
        }
        MintExtensionObservation::PermanentDelegate => {
            MintExtensionCase::PermanentDelegate(payload)
        }
        MintExtensionObservation::TransferFeeConfig => {
            MintExtensionCase::TransferFeeConfig(payload)
        }
        MintExtensionObservation::PausableConfig => MintExtensionCase::PausableConfig(payload),
    }
}

fn token_account_extension_case_for_observation(
    observation: TokenAccountExtensionObservation,
    payload: Vec<u8>,
) -> TokenAccountExtensionCase {
    match observation {
        TokenAccountExtensionObservation::TransferFeeAmount => {
            TokenAccountExtensionCase::TransferFeeAmount(payload)
        }
        TokenAccountExtensionObservation::CpiGuard => TokenAccountExtensionCase::CpiGuard(payload),
        TokenAccountExtensionObservation::TransferHookAccount => {
            TokenAccountExtensionCase::TransferHookAccount(payload)
        }
        TokenAccountExtensionObservation::PausableAccount => {
            TokenAccountExtensionCase::PausableAccount(payload)
        }
    }
}

fn alternate_mint_extension_observation(
    observation: MintExtensionObservation,
) -> MintExtensionObservation {
    match observation {
        MintExtensionObservation::MetadataPointer => MintExtensionObservation::GroupPointer,
        MintExtensionObservation::GroupPointer => MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::GroupMemberPointer => MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::TransferHook => MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::MintCloseAuthority => MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::PermanentDelegate => MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::TransferFeeConfig => MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::PausableConfig => MintExtensionObservation::MetadataPointer,
    }
}

fn alternate_token_account_extension_observation(
    observation: TokenAccountExtensionObservation,
) -> TokenAccountExtensionObservation {
    match observation {
        TokenAccountExtensionObservation::TransferFeeAmount => {
            TokenAccountExtensionObservation::CpiGuard
        }
        TokenAccountExtensionObservation::CpiGuard => {
            TokenAccountExtensionObservation::TransferFeeAmount
        }
        TokenAccountExtensionObservation::TransferHookAccount => {
            TokenAccountExtensionObservation::TransferFeeAmount
        }
        TokenAccountExtensionObservation::PausableAccount => {
            TokenAccountExtensionObservation::TransferFeeAmount
        }
    }
}

fn corrupt_data_strategy(data: Vec<u8>) -> BoxedStrategy<Vec<u8>> {
    if data.is_empty() {
        return Just(data).boxed();
    }
    let len = data.len();
    (
        Just(data),
        prop_oneof![
            3 => 0usize..len,
            1 => Just(<Token2022Mint as Token2022Pack>::LEN),
            1 => Just(SplTokenAccount::LEN),
        ],
        any::<u8>(),
    )
        .prop_map(|(mut data, index, byte)| {
            let index = index % data.len();
            data[index] = byte;
            data
        })
        .boxed()
}

fn token_2022_mint_data(
    mint: Token2022Mint,
    extension_types: &[Token2022ExtensionType],
) -> Vec<u8> {
    let len = Token2022ExtensionType::try_calculate_account_len::<Token2022Mint>(extension_types)
        .unwrap();
    let mut data = vec![0; len];
    Token2022Mint::pack(mint, &mut data[..<Token2022Mint as Token2022Pack>::LEN]).unwrap();
    spl_token_2022_interface::extension::set_account_type::<Token2022Mint>(&mut data).unwrap();

    let mut state = StateWithExtensionsMut::<Token2022Mint>::unpack(&mut data).unwrap();
    for extension_type in extension_types {
        match extension_type {
            Token2022ExtensionType::TransferFeeConfig => {
                state.init_extension::<TransferFeeConfig>(true).unwrap();
            }
            Token2022ExtensionType::MintCloseAuthority => {
                state.init_extension::<MintCloseAuthority>(true).unwrap();
            }
            Token2022ExtensionType::ConfidentialTransferMint => {
                state
                    .init_extension::<ConfidentialTransferMint>(true)
                    .unwrap();
            }
            Token2022ExtensionType::DefaultAccountState => {
                state.init_extension::<DefaultAccountState>(true).unwrap();
            }
            Token2022ExtensionType::NonTransferable => {
                state.init_extension::<NonTransferable>(true).unwrap();
            }
            Token2022ExtensionType::InterestBearingConfig => {
                state.init_extension::<InterestBearingConfig>(true).unwrap();
            }
            Token2022ExtensionType::PermanentDelegate => {
                state.init_extension::<PermanentDelegate>(true).unwrap();
            }
            Token2022ExtensionType::TransferHook => {
                state.init_extension::<TransferHook>(true).unwrap();
            }
            Token2022ExtensionType::ConfidentialTransferFeeConfig => {
                state
                    .init_extension::<ConfidentialTransferFeeConfig>(true)
                    .unwrap();
            }
            Token2022ExtensionType::MetadataPointer => {
                state.init_extension::<MetadataPointer>(true).unwrap();
            }
            Token2022ExtensionType::GroupPointer => {
                state.init_extension::<GroupPointer>(true).unwrap();
            }
            Token2022ExtensionType::TokenGroup => {
                state.init_extension::<TokenGroup>(true).unwrap();
            }
            Token2022ExtensionType::GroupMemberPointer => {
                state.init_extension::<GroupMemberPointer>(true).unwrap();
            }
            Token2022ExtensionType::TokenGroupMember => {
                state.init_extension::<TokenGroupMember>(true).unwrap();
            }
            Token2022ExtensionType::ConfidentialMintBurn => {
                state.init_extension::<ConfidentialMintBurn>(true).unwrap();
            }
            Token2022ExtensionType::ScaledUiAmount => {
                state.init_extension::<ScaledUiAmountConfig>(true).unwrap();
            }
            Token2022ExtensionType::Pausable => {
                state.init_extension::<PausableConfig>(true).unwrap();
            }
            _ => unreachable!(),
        }
    }

    data
}

fn token_2022_mint_data_with_extension_cases(
    mint: Token2022Mint,
    extensions: &[MintExtensionCase],
) -> Vec<u8> {
    let extension_types = extensions
        .iter()
        .map(MintExtensionCase::extension_type)
        .collect::<Vec<_>>();
    let len = Token2022ExtensionType::try_calculate_account_len::<Token2022Mint>(&extension_types)
        .unwrap();
    let mut data = vec![0; len];
    Token2022Mint::pack(mint, &mut data[..<Token2022Mint as Token2022Pack>::LEN]).unwrap();
    spl_token_2022_interface::extension::set_account_type::<Token2022Mint>(&mut data).unwrap();

    let mut state = StateWithExtensionsMut::<Token2022Mint>::unpack(&mut data).unwrap();
    for extension in extensions {
        extension.init(&mut state);
    }

    data
}

fn token_2022_mint_token_metadata_data(mint: Token2022Mint, metadata: &[u8]) -> Vec<u8> {
    const BASE_ACCOUNT_LEN: usize = 165;
    const ACCOUNT_TYPE_LEN: usize = 1;
    const TLV_HEADER_LEN: usize = 4;

    let mut data = vec![0; BASE_ACCOUNT_LEN + ACCOUNT_TYPE_LEN + TLV_HEADER_LEN + metadata.len()];
    Token2022Mint::pack(mint, &mut data[..<Token2022Mint as Token2022Pack>::LEN]).unwrap();
    spl_token_2022_interface::extension::set_account_type::<Token2022Mint>(&mut data).unwrap();

    let tlv_offset = BASE_ACCOUNT_LEN + ACCOUNT_TYPE_LEN;
    data[tlv_offset..tlv_offset + 2]
        .copy_from_slice(&(Token2022ExtensionType::TokenMetadata as u16).to_le_bytes());
    data[tlv_offset + 2..tlv_offset + 4].copy_from_slice(&(metadata.len() as u16).to_le_bytes());
    data[tlv_offset + 4..].copy_from_slice(metadata);
    data
}

fn token_2022_token_account_data(
    account: Token2022Account,
    extension_types: &[Token2022ExtensionType],
) -> Vec<u8> {
    let len =
        Token2022ExtensionType::try_calculate_account_len::<Token2022Account>(extension_types)
            .unwrap();
    let mut data = vec![0; len];
    Token2022Account::pack(
        account,
        &mut data[..<Token2022Account as Token2022Pack>::LEN],
    )
    .unwrap();
    spl_token_2022_interface::extension::set_account_type::<Token2022Account>(&mut data).unwrap();

    let mut state = StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).unwrap();
    for extension_type in extension_types {
        match extension_type {
            Token2022ExtensionType::TransferFeeAmount => {
                state.init_extension::<TransferFeeAmount>(true).unwrap();
            }
            Token2022ExtensionType::ConfidentialTransferAccount => {
                state
                    .init_extension::<ConfidentialTransferAccount>(true)
                    .unwrap();
            }
            Token2022ExtensionType::ImmutableOwner => {
                state.init_extension::<ImmutableOwner>(true).unwrap();
            }
            Token2022ExtensionType::MemoTransfer => {
                state.init_extension::<MemoTransfer>(true).unwrap();
            }
            Token2022ExtensionType::CpiGuard => {
                state.init_extension::<CpiGuard>(true).unwrap();
            }
            Token2022ExtensionType::NonTransferableAccount => {
                state
                    .init_extension::<NonTransferableAccount>(true)
                    .unwrap();
            }
            Token2022ExtensionType::TransferHookAccount => {
                state.init_extension::<TransferHookAccount>(true).unwrap();
            }
            Token2022ExtensionType::ConfidentialTransferFeeAmount => {
                state
                    .init_extension::<ConfidentialTransferFeeAmount>(true)
                    .unwrap();
            }
            Token2022ExtensionType::PausableAccount => {
                state.init_extension::<PausableAccount>(true).unwrap();
            }
            _ => unreachable!(),
        }
    }

    data
}

fn token_2022_token_account_data_with_extension_cases(
    account: Token2022Account,
    extensions: &[TokenAccountExtensionCase],
) -> Vec<u8> {
    let extension_types = extensions
        .iter()
        .map(TokenAccountExtensionCase::extension_type)
        .collect::<Vec<_>>();
    let len =
        Token2022ExtensionType::try_calculate_account_len::<Token2022Account>(&extension_types)
            .unwrap();
    let mut data = vec![0; len];
    Token2022Account::pack(
        account,
        &mut data[..<Token2022Account as Token2022Pack>::LEN],
    )
    .unwrap();
    spl_token_2022_interface::extension::set_account_type::<Token2022Account>(&mut data).unwrap();

    let mut state = StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).unwrap();
    for extension in extensions {
        extension.init(&mut state);
    }

    data
}

impl MintExtensionCase {
    fn extension_type(&self) -> Token2022ExtensionType {
        match self {
            MintExtensionCase::MetadataPointer(_) => Token2022ExtensionType::MetadataPointer,
            MintExtensionCase::GroupPointer(_) => Token2022ExtensionType::GroupPointer,
            MintExtensionCase::GroupMemberPointer(_) => Token2022ExtensionType::GroupMemberPointer,
            MintExtensionCase::TransferHook(_) => Token2022ExtensionType::TransferHook,
            MintExtensionCase::MintCloseAuthority(_) => Token2022ExtensionType::MintCloseAuthority,
            MintExtensionCase::PermanentDelegate(_) => Token2022ExtensionType::PermanentDelegate,
            MintExtensionCase::TransferFeeConfig(_) => Token2022ExtensionType::TransferFeeConfig,
            MintExtensionCase::PausableConfig(_) => Token2022ExtensionType::Pausable,
        }
    }

    fn init(&self, state: &mut StateWithExtensionsMut<Token2022Mint>) {
        match self {
            MintExtensionCase::MetadataPointer(payload) => {
                init_extension_bytes::<Token2022Mint, MetadataPointer>(state, payload)
            }
            MintExtensionCase::GroupPointer(payload) => {
                init_extension_bytes::<Token2022Mint, GroupPointer>(state, payload)
            }
            MintExtensionCase::GroupMemberPointer(payload) => {
                init_extension_bytes::<Token2022Mint, GroupMemberPointer>(state, payload)
            }
            MintExtensionCase::TransferHook(payload) => {
                init_extension_bytes::<Token2022Mint, TransferHook>(state, payload)
            }
            MintExtensionCase::MintCloseAuthority(payload) => {
                init_extension_bytes::<Token2022Mint, MintCloseAuthority>(state, payload)
            }
            MintExtensionCase::PermanentDelegate(payload) => {
                init_extension_bytes::<Token2022Mint, PermanentDelegate>(state, payload)
            }
            MintExtensionCase::TransferFeeConfig(payload) => {
                init_extension_bytes::<Token2022Mint, TransferFeeConfig>(state, payload)
            }
            MintExtensionCase::PausableConfig(payload) => {
                init_extension_bytes::<Token2022Mint, PausableConfig>(state, payload)
            }
        }
    }
}

impl TokenAccountExtensionCase {
    fn extension_type(&self) -> Token2022ExtensionType {
        match self {
            TokenAccountExtensionCase::TransferFeeAmount(_) => {
                Token2022ExtensionType::TransferFeeAmount
            }
            TokenAccountExtensionCase::CpiGuard(_) => Token2022ExtensionType::CpiGuard,
            TokenAccountExtensionCase::TransferHookAccount(_) => {
                Token2022ExtensionType::TransferHookAccount
            }
            TokenAccountExtensionCase::PausableAccount(_) => {
                Token2022ExtensionType::PausableAccount
            }
        }
    }

    fn init(&self, state: &mut StateWithExtensionsMut<Token2022Account>) {
        match self {
            TokenAccountExtensionCase::TransferFeeAmount(payload) => {
                init_extension_bytes::<Token2022Account, TransferFeeAmount>(state, payload)
            }
            TokenAccountExtensionCase::CpiGuard(payload) => {
                init_extension_bytes::<Token2022Account, CpiGuard>(state, payload)
            }
            TokenAccountExtensionCase::TransferHookAccount(payload) => {
                init_extension_bytes::<Token2022Account, TransferHookAccount>(state, payload)
            }
            TokenAccountExtensionCase::PausableAccount(payload) => {
                init_extension_bytes::<Token2022Account, PausableAccount>(state, payload)
            }
        }
    }
}

fn init_extension_bytes<S, T>(state: &mut StateWithExtensionsMut<S>, payload: &[u8])
where
    S: spl_token_2022_interface::extension::BaseState,
    T: spl_token_2022_interface::extension::Extension + anchor_lang::bytemuck::Pod + Default,
{
    state.init_extension::<T>(true).unwrap();
    let extension_data = state.get_extension_bytes_mut::<T>().unwrap();
    let copy_len = extension_data.len().min(payload.len());
    extension_data[..copy_len].copy_from_slice(&payload[..copy_len]);
}

fn malformed_mint_coption_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    (0usize..2, invalid_coption_tag_strategy()).prop_map(|(which, tag)| {
        let mut data = vec![0; SplMint::LEN];
        data[36 + 8 + 1] = 1;
        let offset = if which == 0 { 0 } else { 36 + 8 + 1 + 1 };
        data[offset..offset + 4].copy_from_slice(&tag);
        data
    })
}

fn malformed_token_coption_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    (0usize..3, invalid_coption_tag_strategy()).prop_map(|(which, tag)| {
        let mut data = vec![0; SplTokenAccount::LEN];
        data[32 + 32 + 8 + 4 + 32] = 1;
        let offset = match which {
            0 => 32 + 32 + 8,
            1 => 32 + 32 + 8 + 4 + 32 + 1,
            _ => 32 + 32 + 8 + 4 + 32 + 1 + 4 + 8 + 8,
        };
        data[offset..offset + 4].copy_from_slice(&tag);
        data
    })
}

fn invalid_coption_tag_strategy() -> impl Strategy<Value = [u8; 4]> {
    prop_oneof![
        (2u8..=255, any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(a, b, c, d)| [a, b, c, d]),
        (Just(1u8), 1u8..=255, any::<u8>(), any::<u8>()).prop_map(|(a, b, c, d)| [a, b, c, d]),
        (Just(0u8), 1u8..=255, any::<u8>(), any::<u8>()).prop_map(|(a, b, c, d)| [a, b, c, d]),
    ]
}

fn assert_v1_v2_equivalent(deploy_dir: &std::path::Path, case: &Case) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        run_tx(deploy_dir, OracleVersion::V1, case),
        run_tx(deploy_dir, OracleVersion::V2, case),
    );
    Ok(())
}

fn deterministic_output_case(operation: Operation, target: AccountCase) -> Case {
    Case {
        operation,
        payer_lamports: DEFAULT_PAYER_LAMPORTS,
        target,
        output: OutputAccountCase {
            lamports: DEFAULT_ACCOUNT_LAMPORTS,
            data: vec![0; OBSERVATION_LEN],
            owner: OutputOwner::Oracle,
            executable: false,
            rent_epoch: 0,
        },
        target_signer: false,
        target_writable: false,
        output_signer: false,
        output_writable: true,
    }
}

fn deterministic_mint() -> Token2022Mint {
    Token2022Mint {
        mint_authority: Token2022COption::Some(Pubkey::new_from_array([1; 32])),
        supply: 123,
        decimals: 6,
        is_initialized: true,
        freeze_authority: Token2022COption::None,
    }
}

fn deterministic_token_account() -> Token2022Account {
    Token2022Account {
        mint: Pubkey::new_from_array([2; 32]),
        owner: Pubkey::new_from_array([3; 32]),
        amount: 456,
        delegate: Token2022COption::None,
        state: Token2022AccountState::Initialized,
        is_native: Token2022COption::None,
        delegated_amount: 0,
        close_authority: Token2022COption::None,
    }
}

fn account_case(owner: Pubkey, data: Vec<u8>) -> AccountCase {
    AccountCase {
        lamports: DEFAULT_ACCOUNT_LAMPORTS,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn expect_output(case: &Case, expected_tag: u8, expected_operation: u8, expected_status: u8) {
    let deploy_dir = setup();
    let v1 = run_tx(&deploy_dir, OracleVersion::V1, case);
    let v2 = run_tx(&deploy_dir, OracleVersion::V2, case);
    assert_eq!(v1, v2, "v1 and v2 SPL observations diverged");
    let NormalizedResult::Ok(accounts) = v2 else {
        panic!("expected deterministic SPL observation to succeed");
    };
    let out = &accounts[2].data;
    assert_eq!(out[0], expected_tag);
    assert_eq!(out[1], expected_operation);
    assert_eq!(out[2], expected_status);
}

#[test]
fn interface_extension_readers_report_found_extensions() {
    for observation in [
        MintExtensionObservation::MetadataPointer,
        MintExtensionObservation::GroupPointer,
        MintExtensionObservation::GroupMemberPointer,
        MintExtensionObservation::TransferHook,
        MintExtensionObservation::MintCloseAuthority,
        MintExtensionObservation::PermanentDelegate,
        MintExtensionObservation::TransferFeeConfig,
        MintExtensionObservation::PausableConfig,
    ] {
        let target = account_case(
            token_2022_program_id(),
            token_2022_mint_data_with_extension_cases(
                deterministic_mint(),
                &[mint_extension_case_for_observation(
                    observation,
                    vec![7; 128],
                )],
            ),
        );
        let case =
            deterministic_output_case(Operation::InterfaceMintExtension(observation), target);
        expect_output(
            &case,
            TAG_INTERFACE_MINT_EXTENSION,
            observation as u8,
            EXTENSION_STATUS_FOUND,
        );
    }

    for observation in [
        TokenAccountExtensionObservation::TransferFeeAmount,
        TokenAccountExtensionObservation::CpiGuard,
        TokenAccountExtensionObservation::TransferHookAccount,
        TokenAccountExtensionObservation::PausableAccount,
    ] {
        let target = account_case(
            token_2022_program_id(),
            token_2022_token_account_data_with_extension_cases(
                deterministic_token_account(),
                &[token_account_extension_case_for_observation(
                    observation,
                    vec![9; 128],
                )],
            ),
        );
        let case = deterministic_output_case(
            Operation::InterfaceTokenAccountExtension(observation),
            target,
        );
        expect_output(
            &case,
            TAG_INTERFACE_TOKEN_EXTENSION,
            observation as u8,
            EXTENSION_STATUS_FOUND,
        );
    }
}

#[test]
fn interface_extension_readers_report_missing_and_illegal_owner() {
    let missing_mint = deterministic_output_case(
        Operation::InterfaceMintExtension(MintExtensionObservation::TransferFeeConfig),
        account_case(
            token_2022_program_id(),
            token_2022_mint_data_with_extension_cases(
                deterministic_mint(),
                &[MintExtensionCase::MetadataPointer(vec![0; 128])],
            ),
        ),
    );
    expect_output(
        &missing_mint,
        TAG_INTERFACE_MINT_EXTENSION,
        MintExtensionObservation::TransferFeeConfig as u8,
        EXTENSION_STATUS_ACCESS_ERROR,
    );

    let illegal_mint_owner = deterministic_output_case(
        Operation::InterfaceMintExtension(MintExtensionObservation::MetadataPointer),
        account_case(
            token_program_id(),
            token_2022_mint_data_with_extension_cases(
                deterministic_mint(),
                &[MintExtensionCase::MetadataPointer(vec![0; 128])],
            ),
        ),
    );
    expect_output(
        &illegal_mint_owner,
        TAG_INTERFACE_MINT_EXTENSION,
        MintExtensionObservation::MetadataPointer as u8,
        EXTENSION_STATUS_ILLEGAL_OWNER,
    );

    let missing_token_account = deterministic_output_case(
        Operation::InterfaceTokenAccountExtension(
            TokenAccountExtensionObservation::PausableAccount,
        ),
        account_case(
            token_2022_program_id(),
            token_2022_token_account_data_with_extension_cases(
                deterministic_token_account(),
                &[TokenAccountExtensionCase::TransferFeeAmount(vec![0; 128])],
            ),
        ),
    );
    expect_output(
        &missing_token_account,
        TAG_INTERFACE_TOKEN_EXTENSION,
        TokenAccountExtensionObservation::PausableAccount as u8,
        EXTENSION_STATUS_ACCESS_ERROR,
    );

    let illegal_token_account_owner = deterministic_output_case(
        Operation::InterfaceTokenAccountExtension(TokenAccountExtensionObservation::CpiGuard),
        account_case(
            token_program_id(),
            token_2022_token_account_data_with_extension_cases(
                deterministic_token_account(),
                &[TokenAccountExtensionCase::CpiGuard(vec![0; 128])],
            ),
        ),
    );
    expect_output(
        &illegal_token_account_owner,
        TAG_INTERFACE_TOKEN_EXTENSION,
        TokenAccountExtensionObservation::CpiGuard as u8,
        EXTENSION_STATUS_ILLEGAL_OWNER,
    );
}

fn equivalence_proptest_cases() -> u32 {
    std::env::var("ANCHOR_EQUIVALENCE_PROPTEST_CASES")
        .ok()
        .and_then(|cases| cases.parse().ok())
        .unwrap_or(32 * 1024)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: equivalence_proptest_cases(),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    #[ignore = "runs a fuzz-style SPL v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn spl_v1_v2_strict_mint_has_equivalent_tx_results(
        case in mint_case_strategy(Operation::StrictMint, strict_owner_strategy()),
    ) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }

    #[test]
    #[ignore = "runs a fuzz-style SPL v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn spl_v1_v2_strict_token_account_has_equivalent_tx_results(
        case in token_account_case_strategy(Operation::StrictTokenAccount, strict_owner_strategy()),
    ) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }

    #[test]
    #[ignore = "runs a fuzz-style SPL v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn spl_v1_v2_interface_mint_has_equivalent_tx_results(
        case in mint_case_strategy(Operation::InterfaceMint, interface_owner_strategy()),
    ) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }

    #[test]
    #[ignore = "runs a fuzz-style SPL v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn spl_v1_v2_interface_token_account_has_equivalent_tx_results(
        case in token_account_case_strategy(Operation::InterfaceTokenAccount, interface_owner_strategy()),
    ) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }

    #[test]
    #[ignore = "runs a fuzz-style SPL v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn spl_v1_v2_interface_mint_extensions_have_equivalent_tx_results(
        case in mint_extension_case_strategy(),
    ) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }

    #[test]
    #[ignore = "runs a fuzz-style SPL v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn spl_v1_v2_interface_token_account_extensions_have_equivalent_tx_results(
        case in token_account_extension_case_strategy(),
    ) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }
}
