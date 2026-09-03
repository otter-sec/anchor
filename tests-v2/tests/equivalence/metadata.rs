use {
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    litesvm::LiteSVM,
    proptest::prelude::*,
    sha2::{Digest, Sha256},
    solana_account::Account,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::sync::OnceLock,
    tests_v2::{build_program, keypair_for},
};

const MAX_SMALL_RANDOM_DATA_LEN: usize = 512;
const MAX_LARGE_RANDOM_DATA_LEN: usize = 20 * 1024;
const DEFAULT_ACCOUNT_LAMPORTS: u64 = 10_000_000;
const DEFAULT_PAYER_LAMPORTS: u64 = 10_000_000_000;

#[derive(Clone, Copy, Debug)]
enum OracleVersion {
    V1,
    V2,
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    SignMetadata,
    RemoveCreatorVerification,
    UpdatePrimarySaleHappenedViaToken,
    SetTokenStandard,
}

impl Operation {
    fn v1_name(self) -> &'static str {
        match self {
            Operation::SignMetadata => "sign_metadata",
            Operation::RemoveCreatorVerification => "remove_creator_verification",
            Operation::UpdatePrimarySaleHappenedViaToken => {
                "update_primary_sale_happened_via_token"
            }
            Operation::SetTokenStandard => "set_token_standard",
        }
    }

    fn v2_discriminator(self) -> u8 {
        match self {
            Operation::SignMetadata => 0,
            Operation::RemoveCreatorVerification => 1,
            Operation::UpdatePrimarySaleHappenedViaToken => 2,
            Operation::SetTokenStandard => 3,
        }
    }

    fn roles(self) -> &'static [Role] {
        match self {
            Operation::SignMetadata | Operation::RemoveCreatorVerification => {
                &[Role::Metadata, Role::Creator]
            }
            Operation::UpdatePrimarySaleHappenedViaToken => {
                &[Role::Metadata, Role::Owner, Role::Token]
            }
            Operation::SetTokenStandard => &[Role::Metadata, Role::UpdateAuthority, Role::Mint],
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Role {
    Metadata,
    Creator,
    Owner,
    Token,
    UpdateAuthority,
    Mint,
}

#[derive(Clone, Debug)]
struct Case {
    operation: Operation,
    payer_lamports: u64,
    metadata: RoleCase,
    creator: RoleCase,
    owner: RoleCase,
    token: RoleCase,
    update_authority: RoleCase,
    mint: RoleCase,
}

impl Case {
    fn role_case(&self, role: Role) -> &RoleCase {
        match role {
            Role::Metadata => &self.metadata,
            Role::Creator => &self.creator,
            Role::Owner => &self.owner,
            Role::Token => &self.token,
            Role::UpdateAuthority => &self.update_authority,
            Role::Mint => &self.mint,
        }
    }
}

#[derive(Clone, Debug)]
struct RoleCase {
    lamports: u64,
    data: Vec<u8>,
    owner: Pubkey,
    executable: bool,
    rent_epoch: u64,
    signer: bool,
    writable: bool,
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
    "8HTToHNV33ciwFpvw5t1BKWhnHkQP58LVXLzEtdSS8qi"
        .parse()
        .unwrap()
}

fn v2_program_id() -> Pubkey {
    "AnQXhs18cC2Q6xqUhPoov4DiYiypsbfY95Mcuy37ZHe5"
        .parse()
        .unwrap()
}

fn metadata_program_id() -> Pubkey {
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
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
                    .join("programs/equivalence/metadata/v1")
                    .to_str()
                    .unwrap(),
                deploy_dir.to_str().unwrap(),
            );
            build_program(
                test_dir
                    .join("programs/equivalence/metadata/v2")
                    .to_str()
                    .unwrap(),
                deploy_dir.to_str().unwrap(),
            );
            build_program(
                test_dir
                    .join("programs/equivalence/metadata/spy")
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
    let payer = keypair_for("metadata-equivalence-payer");
    if svm.airdrop(&payer.pubkey(), case.payer_lamports).is_err() {
        return NormalizedResult::Err;
    }

    svm.add_program_from_file(
        metadata_program_id(),
        deploy_dir.join("equivalence_metadata_spy.so"),
    )
    .expect("load metadata spy");

    let program_id = match version {
        OracleVersion::V1 => {
            let id = v1_program_id();
            svm.add_program_from_file(id, deploy_dir.join("equivalence_metadata_v1.so"))
                .expect("load v1 oracle");
            id
        }
        OracleVersion::V2 => {
            let id = v2_program_id();
            svm.add_program_from_file(id, deploy_dir.join("equivalence_metadata_v2.so"))
                .expect("load v2 oracle");
            id
        }
    };

    for role in [
        Role::Metadata,
        Role::Creator,
        Role::Owner,
        Role::Token,
        Role::UpdateAuthority,
        Role::Mint,
    ] {
        if seed_role_account(&mut svm, role, case.role_case(role)).is_err() {
            return NormalizedResult::Err;
        }
    }

    let instruction_data = match version {
        OracleVersion::V1 => v1_instruction_data(case.operation),
        OracleVersion::V2 => vec![case.operation.v2_discriminator()],
    };

    let mut metas = vec![AccountMeta::new_readonly(metadata_program_id(), false)];
    for role in case.operation.roles() {
        let role_case = case.role_case(*role);
        metas.push(AccountMeta {
            pubkey: role_pubkey(*role),
            is_signer: role_case.signer,
            is_writable: role_case.writable,
        });
    }

    let instruction = Instruction::new_with_bytes(program_id, &instruction_data, metas);
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let extra_signers = signer_keypairs(case);
    let mut signers: Vec<&dyn Signer> = vec![&payer];
    signers.extend(extra_signers.iter().map(|signer| signer as &dyn Signer));
    let transaction =
        match VersionedTransaction::try_new(VersionedMessage::Legacy(message), &signers) {
            Ok(transaction) => transaction,
            Err(_) => return NormalizedResult::Err,
        };

    match svm.send_transaction(transaction) {
        Ok(_) => NormalizedResult::Ok(snapshot_accounts(&svm)),
        Err(_) => NormalizedResult::Err,
    }
}

fn seed_role_account(svm: &mut LiteSVM, role: Role, case: &RoleCase) -> Result<(), ()> {
    svm.set_account(
        role_pubkey(role),
        Account {
            lamports: case.lamports,
            data: case.data.clone(),
            owner: case.owner,
            executable: case.executable,
            rent_epoch: case.rent_epoch,
        },
    )
    .map_err(|_| ())
}

fn snapshot_accounts(svm: &LiteSVM) -> Vec<NormalizedAccount> {
    [
        Role::Metadata,
        Role::Creator,
        Role::Owner,
        Role::Token,
        Role::UpdateAuthority,
        Role::Mint,
    ]
    .into_iter()
    .map(|role| {
        let key = role_pubkey(role);
        match svm.get_account(&key) {
            Some(account) => NormalizedAccount {
                key,
                lamports: account.lamports,
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                data: account.data,
            },
            None => NormalizedAccount {
                key,
                lamports: 0,
                owner: solana_sdk_ids::system_program::ID,
                executable: false,
                rent_epoch: 0,
                data: Vec::new(),
            },
        }
    })
    .collect()
}

fn v1_instruction_data(operation: Operation) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", operation.v1_name()).as_bytes());
    hasher.finalize()[..8].to_vec()
}

fn signer_keypairs(case: &Case) -> Vec<solana_keypair::Keypair> {
    [
        Role::Metadata,
        Role::Creator,
        Role::Owner,
        Role::Token,
        Role::UpdateAuthority,
        Role::Mint,
    ]
    .into_iter()
    .filter(|role| case.role_case(*role).signer)
    .map(role_keypair)
    .collect()
}

fn role_pubkey(role: Role) -> Pubkey {
    role_keypair(role).pubkey()
}

fn role_keypair(role: Role) -> solana_keypair::Keypair {
    match role {
        Role::Metadata => keypair_for("metadata-equivalence-metadata"),
        Role::Creator => keypair_for("metadata-equivalence-creator"),
        Role::Owner => keypair_for("metadata-equivalence-owner"),
        Role::Token => keypair_for("metadata-equivalence-token"),
        Role::UpdateAuthority => keypair_for("metadata-equivalence-update-authority"),
        Role::Mint => keypair_for("metadata-equivalence-mint"),
    }
}

fn case_strategy() -> impl Strategy<Value = Case> {
    (
        operation_strategy(),
        payer_lamports_strategy(),
        role_case_strategy(),
        role_case_strategy(),
        role_case_strategy(),
        role_case_strategy(),
        role_case_strategy(),
        role_case_strategy(),
    )
        .prop_map(
            |(
                operation,
                payer_lamports,
                metadata,
                creator,
                owner,
                token,
                update_authority,
                mint,
            )| Case {
                operation,
                payer_lamports,
                metadata,
                creator,
                owner,
                token,
                update_authority,
                mint,
            },
        )
}

fn operation_strategy() -> impl Strategy<Value = Operation> {
    prop_oneof![
        Just(Operation::SignMetadata),
        Just(Operation::RemoveCreatorVerification),
        Just(Operation::UpdatePrimarySaleHappenedViaToken),
        Just(Operation::SetTokenStandard),
    ]
}

fn role_case_strategy() -> impl Strategy<Value = RoleCase> {
    (
        account_lamports_strategy(),
        arbitrary_data_strategy(),
        owner_strategy(),
        executable_strategy(),
        rent_epoch_strategy(),
        signer_strategy(),
        writable_strategy(),
    )
        .prop_map(
            |(lamports, data, owner, executable, rent_epoch, signer, writable)| RoleCase {
                lamports,
                data,
                owner,
                executable,
                rent_epoch,
                signer,
                writable,
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

fn arbitrary_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        8 => prop::collection::vec(any::<u8>(), 0..MAX_SMALL_RANDOM_DATA_LEN),
        1 => prop::collection::vec(any::<u8>(), 0..MAX_LARGE_RANDOM_DATA_LEN),
    ]
}

fn owner_strategy() -> impl Strategy<Value = Pubkey> {
    prop_oneof![
        1 => Just(solana_sdk_ids::system_program::ID),
        1 => Just(metadata_program_id()),
        1 => any::<[u8; 32]>().prop_map(Pubkey::new_from_array),
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
        6 => Just(false),
        3 => Just(true),
    ]
}

fn writable_strategy() -> impl Strategy<Value = bool> {
    prop_oneof![
        6 => Just(true),
        3 => Just(false),
    ]
}

fn rent_epoch_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        6 => Just(0),
        1 => 1u64..1_000_000,
        1 => any::<u64>(),
    ]
}

fn assert_v1_v2_equivalent(deploy_dir: &std::path::Path, case: &Case) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        run_tx(deploy_dir, OracleVersion::V1, case),
        run_tx(deploy_dir, OracleVersion::V2, case),
    );
    Ok(())
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
    #[ignore = "runs a fuzz-style metadata CPI v1/v2 equivalence pass; defaults to 32k cases unless ANCHOR_EQUIVALENCE_PROPTEST_CASES is set"]
    fn metadata_v1_v2_cpi_helpers_have_equivalent_tx_results(case in case_strategy()) {
        let deploy_dir = setup();
        assert_v1_v2_equivalent(&deploy_dir, &case)?;
    }
}
