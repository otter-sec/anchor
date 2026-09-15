use {
    crate::cases::{self, AccountKind, CASES},
    anyhow::{Context, Result},
    indexmap::IndexMap,
    litesvm::LiteSVM,
    sha2::{Digest, Sha256},
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::{fs, path::Path, str::FromStr},
};

const PROGRAM_ID: &str = "Bench11111111111111111111111111111111111111";
const SYSTEM_ID: &str = "11111111111111111111111111111111";
const TOKEN_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const NATIVE_LOADER_ID: &str = "NativeLoader1111111111111111111111111111111";

pub fn measure(deploy: &Path) -> Result<IndexMap<String, u64>> {
    Harness::new(deploy)?.measure()
}

struct Harness {
    svm: LiteSVM,
    payer: Keypair,
    program: Pubkey,
    system: Pubkey,
    mint: Pubkey,
    token: Pubkey,
}

impl Harness {
    fn new(deploy: &Path) -> Result<Self> {
        let program = key(PROGRAM_ID)?;
        let system = key(SYSTEM_ID)?;
        let token_program = key(TOKEN_ID)?;
        let payer = Keypair::new();
        let mint = Keypair::new().pubkey();
        let token = Keypair::new().pubkey();
        let mut svm = LiteSVM::new();
        svm.add_program(program, &fs::read(deploy)?)?;
        svm.airdrop(&payer.pubkey(), 10_000_000_000)
            .map_err(|error| anyhow::anyhow!("LiteSVM airdrop failed: {error:?}"))?;
        set_account(
            &mut svm,
            token_program,
            key(NATIVE_LOADER_ID)?,
            Vec::new(),
            true,
        )?;
        set_account(
            &mut svm,
            mint,
            token_program,
            mint_data(payer.pubkey()),
            false,
        )?;
        set_account(
            &mut svm,
            token,
            token_program,
            token_data(mint, payer.pubkey()),
            false,
        )?;
        Ok(Self {
            svm,
            payer,
            program,
            system,
            mint,
            token,
        })
    }

    fn measure(mut self) -> Result<IndexMap<String, u64>> {
        let mut results = IndexMap::new();
        for case in CASES {
            for &count in case.counts {
                if case.init {
                    self.measure_instruction(
                        case.kind,
                        &cases::instruction(case.name, true, count),
                        count,
                        true,
                        &mut results,
                    )?;
                }
                self.measure_instruction(
                    case.kind,
                    &cases::instruction(case.name, false, count),
                    count,
                    false,
                    &mut results,
                )?;
            }
        }
        Ok(results)
    }

    fn measure_instruction(
        &mut self,
        kind: AccountKind,
        name: &str,
        count: usize,
        init: bool,
        results: &mut IndexMap<String, u64>,
    ) -> Result<()> {
        let mut metas = Vec::new();
        let mut signers = Vec::new();
        if init {
            metas.extend([
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new_readonly(self.system, false),
            ]);
            for _ in 0..count {
                let signer = Keypair::new();
                metas.push(AccountMeta::new(signer.pubkey(), true));
                signers.push(signer);
            }
        } else {
            for _ in 0..count {
                let address = match kind {
                    AccountKind::Mint => self.mint,
                    AccountKind::Token => self.token,
                    AccountKind::Interface => key(TOKEN_ID)?,
                    AccountKind::Program => self.system,
                    kind => {
                        let signer = Keypair::new();
                        let address = signer.pubkey();
                        let (owner, data) = match kind {
                            AccountKind::Empty => (self.program, anchor_data("Empty", 0)),
                            AccountKind::Sized => (self.program, anchor_data("Sized", 8)),
                            AccountKind::Unsized => (self.program, anchor_data("Unsized", 4)),
                            _ => (self.system, Vec::new()),
                        };
                        set_account(&mut self.svm, address, owner, data, false)?;
                        if matches!(kind, AccountKind::Signer) {
                            signers.push(signer);
                        }
                        address
                    }
                };
                metas.push(AccountMeta::new_readonly(
                    address,
                    matches!(kind, AccountKind::Signer),
                ));
            }
        }

        let instruction =
            Instruction::new_with_bytes(self.program, &discriminator("global", name), metas);
        let message = Message::new_with_blockhash(
            std::slice::from_ref(&instruction),
            Some(&self.payer.pubkey()),
            &self.svm.latest_blockhash(),
        );
        let mut transaction_signers = vec![&self.payer];
        transaction_signers.extend(&signers);
        let transaction =
            VersionedTransaction::try_new(VersionedMessage::Legacy(message), &transaction_signers)?;
        let result = self
            .svm
            .simulate_transaction(transaction)
            .map_err(|error| anyhow::anyhow!("LiteSVM simulation failed for {name}: {error:?}"))?;
        results.insert(cases::result_name(name), result.meta.compute_units_consumed);
        Ok(())
    }
}

fn set_account(
    svm: &mut LiteSVM,
    address: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    executable: bool,
) -> Result<()> {
    svm.set_account(
        address,
        Account {
            lamports: 1_000_000_000,
            data,
            owner,
            executable,
            rent_epoch: 0,
        },
    )?;
    Ok(())
}

fn anchor_data(name: &str, size: usize) -> Vec<u8> {
    let mut data = discriminator("account", name).to_vec();
    data.resize(8 + size, 0);
    data
}

fn mint_data(authority: Pubkey) -> Vec<u8> {
    let mut data = vec![0; 82];
    data[..4].copy_from_slice(&1_u32.to_le_bytes());
    data[4..36].copy_from_slice(authority.as_ref());
    data[45] = 1;
    data
}

fn token_data(mint: Pubkey, owner: Pubkey) -> Vec<u8> {
    let mut data = vec![0; 165];
    data[..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[108] = 1;
    data
}

fn discriminator(namespace: &str, name: &str) -> [u8; 8] {
    Sha256::digest(format!("{namespace}:{name}"))[..8]
        .try_into()
        .unwrap()
}

fn key(value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).with_context(|| format!("Invalid public key {value}"))
}
