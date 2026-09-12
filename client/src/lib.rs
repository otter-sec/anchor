#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! An RPC client to interact with Solana programs written in [`anchor_lang`].
//!
//! # Examples
//!
//! A simple example that creates a client, sends a transaction and fetches an account:
//!
//! ```ignore
//! use std::rc::Rc;
//!
//! use anchor_client::{
//!     solana_sdk::{
//!         signature::{read_keypair_file, Keypair},
//!         signer::Signer,
//!         system_program,
//!     },
//!     Client, Cluster,
//! };
//! use my_program::{accounts, instruction, MyAccount};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let payer = read_keypair_file("keypair.json")?;
//!     let client = Client::new(Cluster::Localnet, Rc::new(payer));
//!
//!     // Create program
//!     let program = client.program(my_program::ID)?;
//!
//!     // Send a transaction
//!     let my_account_kp = Keypair::new();
//!     program
//!         .request()
//!         .accounts(accounts::Initialize {
//!             my_account: my_account_kp.pubkey(),
//!             payer: program.payer(),
//!             system_program: system_program::ID,
//!         })
//!         .args(instruction::Initialize { field: 42 })
//!         .signer(&my_account_kp)
//!         .send()?;
//!
//!     // Fetch an account
//!     let my_account: MyAccount = program.account(my_account_kp.pubkey())?;
//!     assert_eq!(my_account.field, 42);
//!
//!     Ok(())
//! }
//! ```
//!
//! More examples can be found in [here].
//!
//! [here]: https://github.com/coral-xyz/anchor/tree/v0.30.2/client/example/src
//!
//! # Features
//!
//! The client is blocking by default. To enable asynchronous client, add `async` feature:
//!
//! ```toml
//! anchor-client = { version = "0.30.2 ", features = ["async"] }
//! ````

use anchor_lang::solana_program::hash::Hash;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program_error::ProgramError;
use anchor_lang::solana_program::pubkey::Pubkey;
use anchor_lang::{AccountDeserialize, Discriminator, InstructionData, ToAccountMetas};
use futures::{Future, StreamExt};
use regex::Regex;
use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcSendTransactionConfig,
    RpcTransactionLogsConfig, RpcTransactionLogsFilter,
};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_client::{
    client_error::ClientError as SolanaClientError,
    nonblocking::{
        pubsub_client::{PubsubClient, PubsubClientError},
        rpc_client::RpcClient as AsyncRpcClient,
    },
    rpc_client::RpcClient,
    rpc_response::{Response as RpcResponse, RpcLogsResponse},
};
use solana_sdk::account::Account;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Signature, Signer};
use solana_sdk::transaction::Transaction;
use std::iter::Map;
use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::vec::IntoIter;
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver},
        RwLock,
    },
    task::JoinHandle,
};

pub use anchor_lang;
pub use cluster::Cluster;
pub use solana_client;
pub use solana_sdk;

mod cluster;

#[cfg(not(feature = "async"))]
mod blocking;
#[cfg(feature = "async")]
mod nonblocking;

const PROGRAM_LOG: &str = "Program log: ";
const PROGRAM_DATA: &str = "Program data: ";

type UnsubscribeFn = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;
/// Client defines the base configuration for building RPC clients to
/// communicate with Anchor programs running on a Solana cluster. It's
/// primary use is to build a `Program` client via the `program` method.
pub struct Client<C> {
    cfg: Config<C>,
}

impl<C: Clone + Deref<Target = impl Signer>> Client<C> {
    pub fn new(cluster: Cluster, payer: C) -> Self {
        Self {
            cfg: Config {
                cluster,
                payer,
                options: None,
            },
        }
    }

    pub fn new_with_options(cluster: Cluster, payer: C, options: CommitmentConfig) -> Self {
        Self {
            cfg: Config {
                cluster,
                payer,
                options: Some(options),
            },
        }
    }

    pub fn program(&self, program_id: Pubkey) -> Result<Program<C>, ClientError> {
        let cfg = Config {
            cluster: self.cfg.cluster.clone(),
            options: self.cfg.options,
            payer: self.cfg.payer.clone(),
        };

        Program::new(program_id, cfg)
    }
}

/// Auxiliary data structure to align the types of the Solana CLI utils with Anchor client.
/// Client<C> implementation requires <C: Clone + Deref<Target = impl Signer>> which does not comply with Box<dyn Signer>
/// that's used when loaded Signer from keypair file. This struct is used to wrap the usage.
pub struct DynSigner(pub Arc<dyn Signer>);

impl Signer for DynSigner {
    fn pubkey(&self) -> Pubkey {
        self.0.pubkey()
    }

    fn try_pubkey(&self) -> Result<Pubkey, solana_sdk::signer::SignerError> {
        self.0.try_pubkey()
    }

    fn sign_message(&self, message: &[u8]) -> solana_sdk::signature::Signature {
        self.0.sign_message(message)
    }

    fn try_sign_message(
        &self,
        message: &[u8],
    ) -> Result<solana_sdk::signature::Signature, solana_sdk::signer::SignerError> {
        self.0.try_sign_message(message)
    }

    fn is_interactive(&self) -> bool {
        self.0.is_interactive()
    }
}

// Internal configuration for a client.
#[derive(Debug)]
pub struct Config<C> {
    cluster: Cluster,
    payer: C,
    options: Option<CommitmentConfig>,
}

pub struct EventUnsubscriber<'a> {
    handle: JoinHandle<Result<(), ClientError>>,
    rx: UnboundedReceiver<UnsubscribeFn>,
    #[cfg(not(feature = "async"))]
    runtime_handle: &'a Handle,
    _lifetime_marker: PhantomData<&'a Handle>,
}

impl<'a> EventUnsubscriber<'a> {
    async fn unsubscribe_internal(mut self) {
        if let Some(unsubscribe) = self.rx.recv().await {
            unsubscribe().await;
        }

        let _ = self.handle.await;
    }
}

/// Program is the primary client handle to be used to build and send requests.
pub struct Program<C> {
    program_id: Pubkey,
    cfg: Config<C>,
    sub_client: Arc<RwLock<Option<PubsubClient>>>,
    #[cfg(not(feature = "async"))]
    rt: tokio::runtime::Runtime,
}

impl<C: Deref<Target = impl Signer> + Clone> Program<C> {
    pub fn payer(&self) -> Pubkey {
        self.cfg.payer.pubkey()
    }

    /// Returns a request builder.
    pub fn request(&self) -> RequestBuilder<C> {
        RequestBuilder::from(
            self.program_id,
            self.cfg.cluster.url(),
            self.cfg.payer.clone(),
            self.cfg.options,
            #[cfg(not(feature = "async"))]
            self.rt.handle(),
        )
    }

    pub fn id(&self) -> Pubkey {
        self.program_id
    }

    pub fn rpc(&self) -> RpcClient {
        RpcClient::new_with_commitment(
            self.cfg.cluster.url().to_string(),
            self.cfg.options.unwrap_or_default(),
        )
    }

    pub fn async_rpc(&self) -> AsyncRpcClient {
        AsyncRpcClient::new_with_commitment(
            self.cfg.cluster.url().to_string(),
            self.cfg.options.unwrap_or_default(),
        )
    }

    async fn account_internal<T: AccountDeserialize>(
        &self,
        address: Pubkey,
    ) -> Result<T, ClientError> {
        let rpc_client = AsyncRpcClient::new_with_commitment(
            self.cfg.cluster.url().to_string(),
            self.cfg.options.unwrap_or_default(),
        );
        let account = rpc_client
            .get_account_with_commitment(&address, CommitmentConfig::processed())
            .await?
            .value
            .ok_or(ClientError::AccountNotFound)?;
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    async fn accounts_lazy_internal<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<ProgramAccountsIterator<T>, ClientError> {
        let account_type_filter =
            RpcFilterType::Memcmp(Memcmp::new_base58_encoded(0, &T::discriminator()));
        let config = RpcProgramAccountsConfig {
            filters: Some([vec![account_type_filter], filters].concat()),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        };
        Ok(ProgramAccountsIterator {
            inner: self
                .async_rpc()
                .get_program_accounts_with_config(&self.id(), config)
                .await?
                .into_iter()
                .map(|(key, account)| {
                    Ok((key, T::try_deserialize(&mut (&account.data as &[u8]))?))
                }),
        })
    }

    async fn init_sub_client_if_needed(&self) -> Result<(), ClientError> {
        let lock = &self.sub_client;
        let mut client = lock.write().await;

        if client.is_none() {
            let sub_client = PubsubClient::new(self.cfg.cluster.ws_url()).await?;
            *client = Some(sub_client);
        }

        Ok(())
    }

    async fn on_internal<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
        &self,
        f: impl Fn(&EventContext, T) + Send + 'static,
    ) -> Result<
        (
            JoinHandle<Result<(), ClientError>>,
            UnboundedReceiver<UnsubscribeFn>,
        ),
        ClientError,
    > {
        self.init_sub_client_if_needed().await?;
        let (tx, rx) = unbounded_channel::<_>();
        let config = RpcTransactionLogsConfig {
            commitment: self.cfg.options,
        };
        let program_id_str = self.program_id.to_string();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id_str.clone()]);

        let lock = Arc::clone(&self.sub_client);

        let handle = tokio::spawn(async move {
            if let Some(ref client) = *lock.read().await {
                let (mut notifications, unsubscribe) =
                    client.logs_subscribe(filter, config).await?;

                tx.send(unsubscribe).map_err(|e| {
                    ClientError::SolanaClientPubsubError(PubsubClientError::RequestFailed {
                        message: "Unsubscribe failed".to_string(),
                        reason: e.to_string(),
                    })
                })?;

                while let Some(logs) = notifications.next().await {
                    let ctx = EventContext {
                        signature: logs.value.signature.parse().unwrap(),
                        slot: logs.context.slot,
                    };
                    let events = parse_logs_response(logs, &program_id_str);
                    for e in events {
                        f(&ctx, e);
                    }
                }
            }
            Ok::<(), ClientError>(())
        });

        Ok((handle, rx))
    }
}

/// Iterator with items of type (Pubkey, T). Used to lazily deserialize account structs.
/// Wrapper type hides the inner type from usages so the implementation can be changed.
pub struct ProgramAccountsIterator<T> {
    inner: Map<IntoIter<(Pubkey, Account)>, AccountConverterFunction<T>>,
}

/// Function type that accepts solana accounts and returns deserialized anchor accounts
type AccountConverterFunction<T> = fn((Pubkey, Account)) -> Result<(Pubkey, T), ClientError>;

impl<T> Iterator for ProgramAccountsIterator<T> {
    type Item = Result<(Pubkey, T), ClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub fn handle_program_log<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    self_program_str: &str,
    l: &str,
) -> Result<(Option<T>, Option<String>, bool), ClientError> {
    use anchor_lang::__private::base64;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    // Log emitted from the current program.
    if let Some(log) = l
        .strip_prefix(PROGRAM_LOG)
        .or_else(|| l.strip_prefix(PROGRAM_DATA))
    {
        let borsh_bytes = match STANDARD.decode(log) {
            Ok(borsh_bytes) => borsh_bytes,
            _ => {
                #[cfg(feature = "debug")]
                println!("Could not base64 decode log: {}", log);
                return Ok((None, None, false));
            }
        };

        let mut slice: &[u8] = &borsh_bytes[..];
        let disc: [u8; 8] = {
            let mut disc = [0; 8];
            disc.copy_from_slice(&borsh_bytes[..8]);
            slice = &slice[8..];
            disc
        };
        let mut event = None;
        if disc == T::discriminator() {
            let e: T = anchor_lang::AnchorDeserialize::deserialize(&mut slice)
                .map_err(|e| ClientError::LogParseError(e.to_string()))?;
            event = Some(e);
        }
        Ok((event, None, false))
    }
    // System log.
    else {
        let (program, did_pop) = handle_system_log(self_program_str, l);
        Ok((None, program, did_pop))
    }
}

pub fn handle_system_log(this_program_str: &str, log: &str) -> (Option<String>, bool) {
    if log.starts_with(&format!("Program {this_program_str} log:")) {
        (Some(this_program_str.to_string()), false)

        // `Invoke [1]` instructions are pushed to the stack in `parse_logs_response`,
        // so this ensures we only push CPIs to the stack at this stage
    } else if log.contains("invoke") && !log.ends_with("[1]") {
        (Some("cpi".to_string()), false) // Any string will do.
    } else {
        let re = Regex::new(r"^Program (.*) success*$").unwrap();
        if re.is_match(log) {
            (None, true)
        } else {
            (None, false)
        }
    }
}

pub struct Execution {
    stack: Vec<String>,
}

impl Execution {
    pub fn new(logs: &mut &[String]) -> Result<Self, ClientError> {
        let l = &logs[0];
        *logs = &logs[1..];

        let re = Regex::new(r"^Program (.*) invoke.*$").unwrap();
        let c = re
            .captures(l)
            .ok_or_else(|| ClientError::LogParseError(l.to_string()))?;
        let program = c
            .get(1)
            .ok_or_else(|| ClientError::LogParseError(l.to_string()))?
            .as_str()
            .to_string();
        Ok(Self {
            stack: vec![program],
        })
    }

    pub fn program(&self) -> String {
        assert!(!self.stack.is_empty());
        self.stack[self.stack.len() - 1].clone()
    }

    pub fn push(&mut self, new_program: String) {
        self.stack.push(new_program);
    }

    pub fn pop(&mut self) {
        assert!(!self.stack.is_empty());
        self.stack.pop().unwrap();
    }
}

#[derive(Debug)]
pub struct EventContext {
    pub signature: Signature,
    pub slot: u64,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Account not found")]
    AccountNotFound,
    #[error("{0}")]
    AnchorError(#[from] anchor_lang::error::Error),
    #[error("{0}")]
    ProgramError(#[from] ProgramError),
    #[error("{0}")]
    SolanaClientError(#[from] SolanaClientError),
    #[error("{0}")]
    SolanaClientPubsubError(#[from] PubsubClientError),
    #[error("Unable to parse log: {0}")]
    LogParseError(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

/// `RequestBuilder` provides a builder interface to create and send
/// transactions to a cluster.
pub struct RequestBuilder<'a, C> {
    cluster: String,
    program_id: Pubkey,
    accounts: Vec<AccountMeta>,
    options: CommitmentConfig,
    instructions: Vec<Instruction>,
    payer: C,
    // Serialized instruction data for the target RPC.
    instruction_data: Option<Vec<u8>>,
    signers: Vec<&'a dyn Signer>,
    #[cfg(not(feature = "async"))]
    handle: &'a Handle,
}

impl<'a, C: Deref<Target = impl Signer> + Clone> RequestBuilder<'a, C> {
    #[must_use]
    pub fn payer(mut self, payer: C) -> Self {
        self.payer = payer;
        self
    }

    #[must_use]
    pub fn cluster(mut self, url: &str) -> Self {
        self.cluster = url.to_string();
        self
    }

    #[must_use]
    pub fn instruction(mut self, ix: Instruction) -> Self {
        self.instructions.push(ix);
        self
    }

    #[must_use]
    pub fn program(mut self, program_id: Pubkey) -> Self {
        self.program_id = program_id;
        self
    }

    /// Set the accounts to pass to the instruction.
    ///
    /// `accounts` argument can be:
    ///
    /// - Any type that implements [`ToAccountMetas`] trait
    /// - A vector of [`AccountMeta`]s (for remaining accounts)
    ///
    /// Note that the given accounts are appended to the previous list of accounts instead of
    /// overriding the existing ones (if any).
    ///
    /// # Example
    ///
    /// ```ignore
    /// program
    ///     .request()
    ///     // Regular accounts
    ///     .accounts(accounts::Initialize {
    ///         my_account: my_account_kp.pubkey(),
    ///         payer: program.payer(),
    ///         system_program: system_program::ID,
    ///     })
    ///     // Remaining accounts
    ///     .accounts(vec![AccountMeta {
    ///         pubkey: remaining,
    ///         is_signer: true,
    ///         is_writable: true,
    ///     }])
    ///     .args(instruction::Initialize { field: 42 })
    ///     .send()?;
    /// ```
    #[must_use]
    pub fn accounts(mut self, accounts: impl ToAccountMetas) -> Self {
        let mut metas = accounts.to_account_metas(None);
        self.accounts.append(&mut metas);
        self
    }

    #[must_use]
    pub fn options(mut self, options: CommitmentConfig) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub fn args(mut self, args: impl InstructionData) -> Self {
        self.instruction_data = Some(args.data());
        self
    }

    #[must_use]
    pub fn signer(mut self, signer: &'a dyn Signer) -> Self {
        self.signers.push(signer);
        self
    }

    pub fn instructions(&self) -> Result<Vec<Instruction>, ClientError> {
        let mut instructions = self.instructions.clone();
        if let Some(ix_data) = &self.instruction_data {
            instructions.push(Instruction {
                program_id: self.program_id,
                data: ix_data.clone(),
                accounts: self.accounts.clone(),
            });
        }

        Ok(instructions)
    }

    fn signed_transaction_with_blockhash(
        &self,
        latest_hash: Hash,
    ) -> Result<Transaction, ClientError> {
        let instructions = self.instructions()?;
        let mut signers = self.signers.clone();
        signers.push(&*self.payer);

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.payer.pubkey()),
            &signers,
            latest_hash,
        );

        Ok(tx)
    }

    pub fn transaction(&self) -> Result<Transaction, ClientError> {
        let instructions = &self.instructions;
        let tx = Transaction::new_with_payer(instructions, Some(&self.payer.pubkey()));
        Ok(tx)
    }

    async fn signed_transaction_internal(&self) -> Result<Transaction, ClientError> {
        let latest_hash =
            AsyncRpcClient::new_with_commitment(self.cluster.to_owned(), self.options)
                .get_latest_blockhash()
                .await?;
        let tx = self.signed_transaction_with_blockhash(latest_hash)?;

        Ok(tx)
    }

    async fn send_internal(&self) -> Result<Signature, ClientError> {
        let rpc_client = AsyncRpcClient::new_with_commitment(self.cluster.to_owned(), self.options);
        let latest_hash = rpc_client.get_latest_blockhash().await?;
        let tx = self.signed_transaction_with_blockhash(latest_hash)?;

        rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(Into::into)
    }

    async fn send_with_spinner_and_config_internal(
        &self,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        let rpc_client = AsyncRpcClient::new_with_commitment(self.cluster.to_owned(), self.options);
        let latest_hash = rpc_client.get_latest_blockhash().await?;
        let tx = self.signed_transaction_with_blockhash(latest_hash)?;

        rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                rpc_client.commitment(),
                config,
            )
            .await
            .map_err(Into::into)
    }
}

fn parse_logs_response<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    logs: RpcResponse<RpcLogsResponse>,
    program_id_str: &str,
) -> Vec<T> {
    let mut logs = &logs.value.logs[..];
    let mut events: Vec<T> = Vec::new();
    if !logs.is_empty() {
        if let Ok(mut execution) = Execution::new(&mut logs) {
            // Create a new peekable iterator so that we can peek at the next log whilst iterating
            let mut logs_iter = logs.iter().peekable();

            while let Some(l) = logs_iter.next() {
                // Parse the log.
                let (event, new_program, did_pop) = {
                    if program_id_str == execution.program() {
                        handle_program_log(program_id_str, l).unwrap_or_else(|e| {
                            println!("Unable to parse log: {e}");
                            std::process::exit(1);
                        })
                    } else {
                        let (program, did_pop) = handle_system_log(program_id_str, l);
                        (None, program, did_pop)
                    }
                };
                // Emit the event.
                if let Some(e) = event {
                    events.push(e);
                }
                // Switch program context on CPI.
                if let Some(new_program) = new_program {
                    execution.push(new_program);
                }
                // Program returned.
                if did_pop {
                    execution.pop();

                    // If the current iteration popped then it means there was a
                    //`Program x success` log. If the next log in the iteration is
                    // of depth [1] then we're not within a CPI and this is a new instruction.
                    //
                    // We need to ensure that the `Execution` instance is updated with
                    // the next program ID, or else `execution.program()` will cause
                    // a panic during the next iteration.
                    if let Some(&next_log) = logs_iter.peek() {
                        if next_log.ends_with("invoke [1]") {
                            let re = Regex::new(r"^Program (.*) invoke.*$").unwrap();
                            let next_instruction =
                                re.captures(next_log).unwrap().get(1).unwrap().as_str();
                            // Within this if block, there will always be a regex match.
                            // Therefore it's safe to unwrap and the captured program ID
                            // at index 1 can also be safely unwrapped.
                            execution.push(next_instruction.to_string());
                        }
                    };
                }
            }
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use solana_client::rpc_response::RpcResponseContext;

    // Creating a mock struct that implements `anchor_lang::events`
    // for type inference in `test_logs`
    use anchor_lang::prelude::*;
    #[derive(Debug, Clone, Copy)]
    #[event]
    pub struct MockEvent {}

    use super::*;
    #[test]
    fn new_execution() {
        let mut logs: &[String] =
            &["Program 7Y8VDzehoewALqJfyxZYMgYCnMTCDhWuGfJKUvjYWATw invoke [1]".to_string()];
        let exe = Execution::new(&mut logs).unwrap();
        assert_eq!(
            exe.stack[0],
            "7Y8VDzehoewALqJfyxZYMgYCnMTCDhWuGfJKUvjYWATw".to_string()
        );
    }

    #[test]
    fn handle_system_log_pop() {
        let log = "Program 7Y8VDzehoewALqJfyxZYMgYCnMTCDhWuGfJKUvjYWATw success";
        let (program, did_pop) = handle_system_log("asdf", log);
        assert_eq!(program, None);
        assert!(did_pop);
    }

    #[test]
    fn handle_system_log_no_pop() {
        let log = "Program 7swsTUiQ6KUK4uFYquQKg4epFRsBnvbrTf2fZQCa2sTJ qwer";
        let (program, did_pop) = handle_system_log("asdf", log);
        assert_eq!(program, None);
        assert!(!did_pop);
    }

    #[test]
    fn test_parse_logs_response() -> Result<()> {
        // Mock logs received within an `RpcResponse`. These are based on a Jupiter transaction.
        let logs = vec![
          "Program VeryCoolProgram invoke [1]", // Outer instruction #1 starts
          "Program log: Instruction: VeryCoolEvent",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 664387 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program VeryCoolProgram consumed 42417 of 700000 compute units",
          "Program VeryCoolProgram success", // Outer instruction #1 ends
          "Program EvenCoolerProgram invoke [1]", // Outer instruction #2 starts
          "Program log: Instruction: EvenCoolerEvent",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
          "Program log: Instruction: TransferChecked",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6200 of 630919 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt invoke [2]",
          "Program log: Instruction: Swap",
          "Program log: INVARIANT: SWAP",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4736 of 539321 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 531933 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt consumed 84670 of 610768 compute units",
          "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt success",
          "Program EvenCoolerProgram invoke [2]",
          "Program EvenCoolerProgram consumed 2021 of 523272 compute units",
          "Program EvenCoolerProgram success",
          "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt invoke [2]",
          "Program log: Instruction: Swap",
          "Program log: INVARIANT: SWAP",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4736 of 418618 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 411230 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt consumed 102212 of 507607 compute units",
          "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt success",
          "Program EvenCoolerProgram invoke [2]",
          "Program EvenCoolerProgram consumed 2021 of 402569 compute units",
          "Program EvenCoolerProgram success",
          "Program 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP invoke [2]",
          "Program log: Instruction: Swap",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4736 of 371140 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: MintTo",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4492 of 341800 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
          "Program log: Instruction: Transfer",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 334370 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP consumed 57610 of 386812 compute units",
          "Program 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP success",
          "Program EvenCoolerProgram invoke [2]",
          "Program EvenCoolerProgram consumed 2021 of 326438 compute units",
          "Program EvenCoolerProgram success",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
          "Program log: Instruction: TransferChecked",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6173 of 319725 compute units",
          "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
          "Program EvenCoolerProgram consumed 345969 of 657583 compute units",
          "Program EvenCoolerProgram success", // Outer instruction #2 ends
          "Program ComputeBudget111111111111111111111111111111 invoke [1]",
          "Program ComputeBudget111111111111111111111111111111 success",
          "Program ComputeBudget111111111111111111111111111111 invoke [1]",
          "Program ComputeBudget111111111111111111111111111111 success"];

        // Converting to Vec<String> as expected in `RpcLogsResponse`
        let logs: Vec<String> = logs.iter().map(|&l| l.to_string()).collect();

        let program_id_str = "VeryCoolProgram";

        // No events returned here. Just ensuring that the function doesn't panic
        // due an incorrectly emptied stack.
        let _: Vec<MockEvent> = parse_logs_response(
            RpcResponse {
                context: RpcResponseContext::new(0),
                value: RpcLogsResponse {
                    signature: "".to_string(),
                    err: None,
                    logs: logs.to_vec(),
                },
            },
            program_id_str,
        );

        Ok(())
    }
}
