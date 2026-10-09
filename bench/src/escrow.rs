use {
    crate::litesvm::{
        discriminator, key, mint_data, set_account, token_data, NATIVE_LOADER_ID, SYSTEM_ID,
        TOKEN_ID,
    },
    anyhow::Result,
    indexmap::IndexMap,
    litesvm::LiteSVM,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::{keypair_from_seed, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::{fs, path::Path},
};

const PROGRAM_ID: &str = "Fg6PaFpoGXkYsidMpWxTWqkZ6W2BeZ7FEfcYkgMQhgVu";
const AMOUNT: u64 = 1_000_000;

pub fn measure(deploy: &Path) -> Result<IndexMap<String, u64>> {
    Harness::new(deploy)?.measure()
}

struct Harness {
    svm: LiteSVM,
    maker: Keypair,
    taker: Keypair,
    program: Pubkey,
    system: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
    maker_token: Pubkey,
    taker_token: Pubkey,
    escrow: Pubkey,
}

impl Harness {
    fn new(deploy: &Path) -> Result<Self> {
        let maker = keypair(1);
        let taker = keypair(2);
        let program = key(PROGRAM_ID)?;
        let system = key(SYSTEM_ID)?;
        let token_program = key(TOKEN_ID)?;
        let mint = keypair(3).pubkey();
        let maker_token = keypair(4).pubkey();
        let taker_token = keypair(5).pubkey();
        let (escrow, _) = Pubkey::find_program_address(
            &[b"escrow", maker.pubkey().as_ref(), mint.as_ref()],
            &program,
        );
        let mut svm = LiteSVM::new();
        svm.add_program(program, &fs::read(deploy)?)?;
        for signer in [&maker, &taker] {
            svm.airdrop(&signer.pubkey(), 10_000_000_000)
                .map_err(|error| anyhow::anyhow!("LiteSVM airdrop failed: {error:?}"))?;
        }
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
            mint_data(maker.pubkey()),
            false,
        )?;
        set_account(
            &mut svm,
            maker_token,
            token_program,
            token_data(mint, maker.pubkey()),
            false,
        )?;
        set_account(
            &mut svm,
            taker_token,
            token_program,
            token_data(mint, taker.pubkey()),
            false,
        )?;
        Ok(Self {
            svm,
            maker,
            taker,
            program,
            system,
            token_program,
            mint,
            maker_token,
            taker_token,
            escrow,
        })
    }

    fn measure(mut self) -> Result<IndexMap<String, u64>> {
        let mut initialize_data = discriminator("global", "initialize").to_vec();
        initialize_data.extend(AMOUNT.to_le_bytes());
        let initialize = Instruction::new_with_bytes(
            self.program,
            &initialize_data,
            vec![
                AccountMeta::new(self.maker.pubkey(), true),
                AccountMeta::new_readonly(self.mint, false),
                AccountMeta::new_readonly(self.maker_token, false),
                AccountMeta::new(self.escrow, false),
                AccountMeta::new_readonly(self.token_program, false),
                AccountMeta::new_readonly(self.system, false),
            ],
        );
        let initialize_units = send(&mut self.svm, initialize, &self.maker)?;

        let take = Instruction::new_with_bytes(
            self.program,
            &discriminator("global", "take"),
            vec![
                AccountMeta::new_readonly(self.taker.pubkey(), true),
                AccountMeta::new(self.maker.pubkey(), false),
                AccountMeta::new_readonly(self.mint, false),
                AccountMeta::new_readonly(self.maker_token, false),
                AccountMeta::new_readonly(self.taker_token, false),
                AccountMeta::new(self.escrow, false),
                AccountMeta::new_readonly(self.token_program, false),
            ],
        );
        let take_units = send(&mut self.svm, take, &self.taker)?;

        Ok(IndexMap::from([
            ("initialize".into(), initialize_units),
            ("take".into(), take_units),
        ]))
    }
}

fn keypair(seed: u8) -> Keypair {
    keypair_from_seed(&[seed; 32]).expect("valid seed")
}

fn send(svm: &mut LiteSVM, instruction: Instruction, payer: &Keypair) -> Result<u64> {
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );
    let transaction = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer])?;
    let result = svm
        .send_transaction(transaction)
        .map_err(|error| anyhow::anyhow!("LiteSVM transaction failed: {error:?}"))?;
    Ok(result.compute_units_consumed)
}
