#![cfg_attr(docsrs, feature(doc_cfg))]

//! An RPC client to interact with Solana programs written in [`anchor_lang`].
//!
//! # Examples
//!
//! A simple example that creates a client, sends a transaction and fetches an account:
//!
//! ```ignore
//! use std::rc::Rc;
//!
//! use anchor_client::{Client, Cluster, Signer};
//! use my_program::{accounts, instruction, MyAccount};
//! use solana_keypair::{read_keypair_file, Keypair};
//! use solana_system_interface::program as system_program;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let payer = read_keypair_file("keypair.json")?;
//!     let client = Client::new(Cluster::Localnet, Rc::new(payer));
//!
//!     // Create program
//!     let program = client.program(my_program::ID)?;
//!
//!     // Send transaction
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
//!     // Fetch account
//!     let my_account: MyAccount = program.account(my_account_kp.pubkey())?;
//!     assert_eq!(my_account.field, 42);
//!
//!     Ok(())
//! }
//! ```
//!
//! More examples can be found in [here].
//!
//! [here]: https://github.com/otter-sec/anchor/tree/v1.0.2/client/example/src
//!
//! # Features
//!
//! ## `async`
//!
//! The client is blocking by default. To enable asynchronous client, add `async` feature:
//!
//! ```toml
//! anchor-client = { version = "1.0.2", features = ["async"] }
//! ````
//!
//! ## `mock`
//!
//! This feature allows passing in a custom RPC client when creating program instances, which is
//! useful for mocking RPC responses, e.g. via [`RpcClient::new_mock`].
//!
//! [`RpcClient::new_mock`]: https://docs.rs/solana-rpc-client/3.0.0/solana_rpc_client/rpc_client/struct.RpcClient.html#method.new_mock

#[cfg(feature = "async")]
pub use nonblocking::ThreadSafeSigner;
pub use {
    anchor_lang,
    cluster::Cluster,
    solana_commitment_config::CommitmentConfig,
    solana_hash::Hash,
    solana_instruction::Instruction,
    solana_message::AddressLookupTableAccount,
    solana_pubsub_client::nonblocking::pubsub_client::PubsubClientError,
    solana_rpc_client_api::{
        client_error::Error as SolanaClientError, config::RpcSendTransactionConfig,
        filter::RpcFilterType,
    },
    solana_signer::{Signer, SignerError},
    solana_transaction::{versioned::VersionedTransaction, Transaction},
};
use {
    anchor_lang::{
        solana_program::{program_error::ProgramError, pubkey::Pubkey},
        AccountDeserialize, Discriminator, InstructionData, ToAccountMetas,
    },
    futures::{Future, StreamExt},
    regex::Regex,
    solana_account_decoder::{UiAccount, UiAccountEncoding},
    solana_instruction::AccountMeta,
    solana_message::v0,
    solana_pubsub_client::nonblocking::pubsub_client::PubsubClient,
    solana_rpc_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient,
    solana_rpc_client_api::{
        config::{
            RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcTransactionLogsConfig,
            RpcTransactionLogsFilter,
        },
        filter::Memcmp,
        response::{Response as RpcResponse, RpcLogsResponse},
    },
    solana_signature::Signature,
    std::{iter::Map, marker::PhantomData, ops::Deref, pin::Pin, sync::Arc, vec::IntoIter},
    thiserror::Error,
    tokio::{
        runtime::Handle,
        sync::{
            mpsc::{unbounded_channel, UnboundedReceiver},
            OnceCell,
        },
        task::JoinHandle,
    },
};

mod cluster;

/// Specifies which transaction version to use when building transactions.
#[derive(Debug, Clone, Default)]
pub enum TxVersion<'a> {
    /// Legacy transaction format.
    #[default]
    Legacy,
    /// Versioned transaction format (v0) with optional address lookup tables.
    V0(&'a [AddressLookupTableAccount]),
}

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

    pub fn program(
        &self,
        program_id: Pubkey,
        #[cfg(feature = "mock")] rpc_client: AsyncRpcClient,
    ) -> Result<Program<C>, ClientError> {
        let cfg = Config {
            cluster: self.cfg.cluster.clone(),
            options: self.cfg.options,
            payer: self.cfg.payer.clone(),
        };

        Program::new(
            program_id,
            cfg,
            #[cfg(feature = "mock")]
            rpc_client,
        )
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

    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        self.0.try_pubkey()
    }

    fn sign_message(&self, message: &[u8]) -> Signature {
        self.0.sign_message(message)
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
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

impl EventUnsubscriber<'_> {
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
    sub_client: OnceCell<Arc<PubsubClient>>,
    #[cfg(not(feature = "async"))]
    rt: tokio::runtime::Runtime,
    internal_rpc_client: AsyncRpcClient,
}

impl<C: Deref<Target = impl Signer> + Clone> Program<C> {
    pub fn payer(&self) -> Pubkey {
        self.cfg.payer.pubkey()
    }

    pub fn id(&self) -> Pubkey {
        self.program_id
    }

    #[cfg(feature = "mock")]
    pub fn internal_rpc(&self) -> &AsyncRpcClient {
        &self.internal_rpc_client
    }

    async fn account_internal<T: AccountDeserialize>(
        &self,
        address: Pubkey,
    ) -> Result<T, ClientError> {
        let account = self
            .internal_rpc_client
            .get_account_with_commitment(&address, CommitmentConfig::processed())
            .await
            .map_err(Box::new)?
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
            RpcFilterType::Memcmp(Memcmp::new_base58_encoded(0, T::DISCRIMINATOR));
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
                .internal_rpc_client
                .get_program_ui_accounts_with_config(&self.id(), config)
                .await
                .map_err(Box::new)?
                .into_iter()
                .map(|(key, account)| {
                    let data = account
                        .data
                        .decode()
                        .expect("account was fetched with binary encoding");
                    Ok((key, T::try_deserialize(&mut data.as_slice())?))
                }),
        })
    }

    async fn on_internal<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
        &self,
        mut f: impl FnMut(&EventContext, T) + Send + 'static,
    ) -> Result<
        (
            JoinHandle<Result<(), ClientError>>,
            UnboundedReceiver<UnsubscribeFn>,
        ),
        ClientError,
    > {
        let client = self
            .sub_client
            .get_or_try_init(|| async {
                PubsubClient::new(self.cfg.cluster.ws_url())
                    .await
                    .map(Arc::new)
                    .map_err(|e| ClientError::SolanaClientPubsubError(Box::new(e)))
            })
            .await?
            .clone();

        let (tx, rx) = unbounded_channel::<_>();
        let config = RpcTransactionLogsConfig {
            commitment: self.cfg.options,
        };
        let program_id_str = self.program_id.to_string();
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id_str.clone()]);

        let handle = tokio::spawn(async move {
            let (mut notifications, unsubscribe) = client
                .logs_subscribe(filter, config)
                .await
                .map_err(Box::new)?;

            tx.send(unsubscribe).map_err(|e| {
                ClientError::SolanaClientPubsubError(Box::new(PubsubClientError::RequestFailed {
                    message: "Unsubscribe failed".to_string(),
                    reason: e.to_string(),
                }))
            })?;

            while let Some(logs) = notifications.next().await {
                let ctx = EventContext {
                    signature: logs.value.signature.parse().unwrap(),
                    slot: logs.context.slot,
                };
                let events = parse_logs_response(logs, &program_id_str)?;
                for e in events {
                    f(&ctx, e);
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
    inner: Map<IntoIter<(Pubkey, UiAccount)>, AccountConverterFunction<T>>,
}

/// Function type that accepts solana accounts and returns deserialized anchor accounts
type AccountConverterFunction<T> = fn((Pubkey, UiAccount)) -> Result<(Pubkey, T), ClientError>;

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
    use {
        anchor_lang::__private::base64,
        base64::{engine::general_purpose::STANDARD, Engine},
    };

    // Log emitted from the current program.
    if let Some(log) = l
        .strip_prefix(PROGRAM_LOG)
        .or_else(|| l.strip_prefix(PROGRAM_DATA))
    {
        let log_bytes = match STANDARD.decode(log) {
            Ok(log_bytes) => log_bytes,
            _ => {
                #[cfg(feature = "debug")]
                println!("Could not base64 decode log: {}", log);
                return Ok((None, None, false));
            }
        };

        let event = log_bytes
            .starts_with(T::DISCRIMINATOR)
            .then(|| {
                let mut data = &log_bytes[T::DISCRIMINATOR.len()..];
                T::deserialize(&mut data).map_err(|e| ClientError::LogParseError(e.to_string()))
            })
            .transpose()?;

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
        let re = Regex::new(r"^Program ([1-9A-HJ-NP-Za-km-z]+) success$").unwrap();
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

        let re = Regex::new(r"^Program ([1-9A-HJ-NP-Za-km-z]+) invoke \[[\d]+\]$").unwrap();
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
    SolanaClientError(#[from] Box<SolanaClientError>),
    #[error("{0}")]
    SolanaClientPubsubError(#[from] Box<PubsubClientError>),
    #[error("Unable to parse log: {0}")]
    LogParseError(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("{0}")]
    SignerError(#[from] SignerError),
    #[error("{0}")]
    CompileError(#[from] solana_message::CompileError),
    #[error("Expected a legacy transaction but got a versioned transaction")]
    NotLegacyTransaction,
}

pub trait AsSigner {
    fn as_signer(&self) -> &dyn Signer;
}

impl AsSigner for Box<dyn Signer + '_> {
    fn as_signer(&self) -> &dyn Signer {
        self.as_ref()
    }
}

/// `RequestBuilder` provides a builder interface to create and send
/// transactions to a cluster.
pub struct RequestBuilder<'a, C, S: 'a> {
    cluster: String,
    program_id: Pubkey,
    accounts: Vec<AccountMeta>,
    options: CommitmentConfig,
    instructions: Vec<Instruction>,
    payer: C,
    instruction_data: Option<Vec<u8>>,
    signers: Vec<S>,
    #[cfg(not(feature = "async"))]
    handle: &'a Handle,
    internal_rpc_client: &'a AsyncRpcClient,
    _phantom: PhantomData<&'a ()>,
}

// Shared implementation for all RequestBuilders
impl<C: Deref<Target = impl Signer> + Clone, S: AsSigner> RequestBuilder<'_, C, S> {
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

    pub fn instructions(&self) -> Vec<Instruction> {
        let mut instructions = self.instructions.clone();
        if let Some(ix_data) = &self.instruction_data {
            instructions.push(Instruction {
                program_id: self.program_id,
                data: ix_data.clone(),
                accounts: self.accounts.clone(),
            });
        }

        instructions
    }

    /// Build the request into a transaction.
    ///
    /// Note: This will build a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`transaction_versioned`].
    pub fn transaction(&self) -> Transaction {
        let instructions = &self.instructions();
        Transaction::new_with_payer(instructions, Some(&self.payer.pubkey()))
    }

    /// Build an unsigned transaction.
    ///
    /// # Arguments
    ///
    /// * `version` - The transaction version to use ([`TxVersion::Legacy`] or [`TxVersion::V0`]).
    /// * `recent_blockhash` - A recent blockhash to include in the transaction message.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anchor_client::{Client, Cluster, TxVersion};
    /// use anchor_lang::prelude::Pubkey;
    /// use solana_signer::null_signer::NullSigner;
    /// use solana_message::AddressLookupTableAccount;
    /// use solana_message::Hash;
    ///
    /// let payer = NullSigner::new(&Pubkey::default());
    /// let client = Client::new(Cluster::Localnet, std::rc::Rc::new(payer));
    ///
    /// let program = client.program(Pubkey::default()).unwrap();
    /// // Dummy blockhash
    /// let blockhash = Hash::from([0; 32]);
    /// let lookup_table = AddressLookupTableAccount { key: Pubkey::default(), addresses: vec![] };
    ///
    /// let request = program.request();
    /// // Legacy transaction
    /// let tx = request.transaction_versioned(TxVersion::Legacy, blockhash).unwrap();
    ///
    /// // V0 transaction with address lookup tables
    /// let tx = request.transaction_versioned(TxVersion::V0(&[lookup_table]), blockhash).unwrap();
    ///
    /// // V0 transaction without lookup tables
    /// let tx = request.transaction_versioned(TxVersion::V0(&[]), blockhash).unwrap();
    //// ```
    pub fn transaction_versioned(
        &self,
        version: TxVersion<'_>,
        recent_blockhash: Hash,
    ) -> Result<solana_transaction::versioned::VersionedTransaction, ClientError> {
        let instructions = self.instructions();
        let payer = self.payer.pubkey();

        match version {
            TxVersion::Legacy => {
                let message = solana_message::legacy::Message::new_with_blockhash(
                    &instructions,
                    Some(&payer),
                    &recent_blockhash,
                );
                Ok(solana_transaction::versioned::VersionedTransaction {
                    signatures: vec![
                        solana_signature::Signature::default();
                        message.header.num_required_signatures as usize
                    ],
                    message: solana_message::VersionedMessage::Legacy(message),
                })
            }
            TxVersion::V0(address_lookup_table_accounts) => {
                let message = v0::Message::try_compile(
                    &payer,
                    &instructions,
                    address_lookup_table_accounts,
                    recent_blockhash,
                )?;
                Ok(solana_transaction::versioned::VersionedTransaction {
                    signatures: vec![
                        solana_signature::Signature::default();
                        message.header.num_required_signatures as usize
                    ],
                    message: solana_message::VersionedMessage::V0(message),
                })
            }
        }
    }

    fn signed_transaction_with_blockhash_versioned(
        &self,
        version: TxVersion<'_>,
        latest_hash: Hash,
    ) -> Result<solana_transaction::versioned::VersionedTransaction, ClientError> {
        let signers: Vec<&dyn Signer> = self.signers.iter().map(|s| s.as_signer()).collect();
        let mut all_signers = signers;
        all_signers.push(&*self.payer);

        let instructions = self.instructions();
        let payer = self.payer.pubkey();

        let message = match version {
            TxVersion::Legacy => {
                let msg = solana_message::legacy::Message::new_with_blockhash(
                    &instructions,
                    Some(&payer),
                    &latest_hash,
                );
                solana_message::VersionedMessage::Legacy(msg)
            }
            TxVersion::V0(address_lookup_table_accounts) => {
                let msg = v0::Message::try_compile(
                    &payer,
                    &instructions,
                    address_lookup_table_accounts,
                    latest_hash,
                )?;
                solana_message::VersionedMessage::V0(msg)
            }
        };

        let tx =
            solana_transaction::versioned::VersionedTransaction::try_new(message, &all_signers)?;

        Ok(tx)
    }

    async fn signed_transaction_internal(
        &self,
        version: TxVersion<'_>,
    ) -> Result<solana_transaction::versioned::VersionedTransaction, ClientError> {
        let latest_hash = self
            .internal_rpc_client
            .get_latest_blockhash()
            .await
            .map_err(Box::new)?;

        self.signed_transaction_with_blockhash_versioned(version, latest_hash)
    }

    async fn send_internal(&self, version: TxVersion<'_>) -> Result<Signature, ClientError> {
        let latest_hash = self
            .internal_rpc_client
            .get_latest_blockhash()
            .await
            .map_err(Box::new)?;
        let tx = self.signed_transaction_with_blockhash_versioned(version, latest_hash)?;

        self.internal_rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| Box::new(e).into())
    }

    async fn send_with_spinner_and_config_internal(
        &self,
        version: TxVersion<'_>,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        let latest_hash = self
            .internal_rpc_client
            .get_latest_blockhash()
            .await
            .map_err(Box::new)?;
        let tx = self.signed_transaction_with_blockhash_versioned(version, latest_hash)?;

        self.internal_rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                self.internal_rpc_client.commitment(),
                config,
            )
            .await
            .map_err(|e| Box::new(e).into())
    }
}

fn parse_logs_response<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
    logs: RpcResponse<RpcLogsResponse>,
    program_id_str: &str,
) -> Result<Vec<T>, ClientError> {
    let mut logs = &logs.value.logs[..];
    let mut events: Vec<T> = Vec::new();
    if !logs.is_empty() {
        if let Ok(mut execution) = Execution::new(&mut logs) {
            // Create a new peekable iterator so that we can peek at the next log whilst iterating
            let mut logs_iter = logs.iter().peekable();
            let regex = Regex::new(r"^Program ([1-9A-HJ-NP-Za-km-z]+) invoke \[(\d+)\]$").unwrap();

            while let Some(l) = logs_iter.next() {
                // Parse the log.
                let (event, new_program, did_pop) = {
                    if program_id_str == execution.program() {
                        handle_program_log(program_id_str, l)?
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
                    //
                    // Use the full regex match to gate this branch. A loose
                    // `ends_with("invoke [1]")` check would also accept program-emitted
                    // log lines that happen to end in that suffix (e.g.
                    // `"Program log: ...invoke [1]"`), which then fail the strict
                    // `^Program <pubkey> invoke [N]$` regex and panic on unwrap.
                    if let Some(&next_log) = logs_iter.peek() {
                        if let Some(caps) = regex.captures(next_log) {
                            if &caps[2] == "1" {
                                execution.push(caps[1].to_string());
                            }
                        }
                    };
                }
            }
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    // Creating a mock struct that implements `anchor_lang::events`
    // for type inference in `test_logs`
    use {
        anchor_lang::prelude::*,
        futures::{SinkExt, StreamExt},
        solana_rpc_client_api::response::RpcResponseContext,
        std::sync::atomic::{AtomicU64, Ordering},
        tokio_tungstenite::tungstenite::Message,
    };
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
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 664387 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program VeryCoolProgram consumed 42417 of 700000 compute units",
            "Program VeryCoolProgram success", // Outer instruction #1 ends
            "Program EvenCoolerProgram invoke [1]", // Outer instruction #2 starts
            "Program log: Instruction: EvenCoolerEvent",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
            "Program log: Instruction: TransferChecked",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6200 of 630919 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt invoke [2]",
            "Program log: Instruction: Swap",
            "Program log: INVARIANT: SWAP",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: Transfer",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4736 of 539321 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: Transfer",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 531933 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt consumed 84670 of 610768 \
             compute units",
            "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt success",
            "Program EvenCoolerProgram invoke [2]",
            "Program EvenCoolerProgram consumed 2021 of 523272 compute units",
            "Program EvenCoolerProgram success",
            "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt invoke [2]",
            "Program log: Instruction: Swap",
            "Program log: INVARIANT: SWAP",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: Transfer",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4736 of 418618 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: Transfer",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 411230 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt consumed 102212 of 507607 \
             compute units",
            "Program HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt success",
            "Program EvenCoolerProgram invoke [2]",
            "Program EvenCoolerProgram consumed 2021 of 402569 compute units",
            "Program EvenCoolerProgram success",
            "Program 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP invoke [2]",
            "Program log: Instruction: Swap",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: Transfer",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4736 of 371140 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: MintTo",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4492 of 341800 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]",
            "Program log: Instruction: Transfer",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4645 of 334370 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP consumed 57610 of 386812 \
             compute units",
            "Program 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP success",
            "Program EvenCoolerProgram invoke [2]",
            "Program EvenCoolerProgram consumed 2021 of 326438 compute units",
            "Program EvenCoolerProgram success",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
            "Program log: Instruction: TransferChecked",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 6173 of 319725 compute \
             units",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            "Program EvenCoolerProgram consumed 345969 of 657583 compute units",
            "Program EvenCoolerProgram success", // Outer instruction #2 ends
            "Program ComputeBudget111111111111111111111111111111 invoke [1]",
            "Program ComputeBudget111111111111111111111111111111 success",
            "Program ComputeBudget111111111111111111111111111111 invoke [1]",
            "Program ComputeBudget111111111111111111111111111111 success",
        ];

        // Converting to Vec<String> as expected in `RpcLogsResponse`
        let logs: Vec<String> = logs.iter().map(|&l| l.to_string()).collect();

        let program_id_str = "VeryCoolProgram";

        // No events returned here. Just ensuring that the function doesn't panic
        // due an incorrectly emptied stack.
        parse_logs_response::<MockEvent>(
            RpcResponse {
                context: RpcResponseContext::new(0),
                value: RpcLogsResponse {
                    signature: "".to_string(),
                    err: None,
                    logs: logs.to_vec(),
                },
            },
            program_id_str,
        )
        .unwrap();

        Ok(())
    }

    #[test]
    fn test_parse_logs_response_fake_pop() -> Result<()> {
        let logs = [
            "Program fake111111111111111111111111111111111111112 invoke [1]",
            "Program log: i logged success",
            "Program log: i logged success",
            "Program fake111111111111111111111111111111111111112 consumed 1411 of 200000 compute \
             units",
            "Program fake111111111111111111111111111111111111112 success",
        ];

        // Converting to Vec<String> as expected in `RpcLogsResponse`
        let logs: Vec<String> = logs.iter().map(|&l| l.to_string()).collect();

        let program_id_str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

        // No events returned here. Just ensuring that the function doesn't panic
        // due an incorrectly emptied stack.
        parse_logs_response::<MockEvent>(
            RpcResponse {
                context: RpcResponseContext::new(0),
                value: RpcLogsResponse {
                    signature: "".to_string(),
                    err: None,
                    logs: logs.to_vec(),
                },
            },
            program_id_str,
        )
        .unwrap();

        Ok(())
    }

    /// Regression for #4461: a program-emitted `Program log:` line that ends
    /// with the literal `"invoke [1]"` (e.g. log content that happens to
    /// describe a CPI) used to satisfy the `ends_with` gate but fail the
    /// strict `^Program <pubkey> invoke [N]$` regex, panicking on
    /// `.captures(...).unwrap()` whenever it appeared right after a CPI pop.
    #[test]
    fn test_parse_logs_response_log_line_ends_with_invoke_1() -> Result<()> {
        let logs = [
            "Program VeryCoolProgram invoke [1]",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]",
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success",
            // Program-emitted log that happens to end with "invoke [1]"
            // immediately after a CPI returns. Pre-fix this would panic.
            "Program log: forwarded inner instruction invoke [1]",
            "Program VeryCoolProgram success",
        ];
        let logs: Vec<String> = logs.iter().map(|&l| l.to_string()).collect();

        parse_logs_response::<MockEvent>(
            RpcResponse {
                context: RpcResponseContext::new(0),
                value: RpcLogsResponse {
                    signature: "".to_string(),
                    err: None,
                    logs,
                },
            },
            "VeryCoolProgram",
        )
        .unwrap();

        Ok(())
    }

    /// Regression test that registering multiple event listeners does not deadlock.
    #[test]
    fn multiple_listeners_no_deadlock() {
        // Spin up a tiny mock websocket server that responds to `logsSubscribe`
        // JSON-RPC requests with a valid subscription id.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let (addr_tx, addr_rx) = std::sync::mpsc::channel();

        rt.spawn(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            addr_tx.send(addr).unwrap();

            static SUB_ID: AtomicU64 = AtomicU64::new(0);

            loop {
                let (stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                    while let Some(Ok(Message::Text(_))) = ws.next().await {
                        let sub_id = SUB_ID.fetch_add(1, Ordering::Relaxed);
                        // The PubsubClient sends sequential integer ids starting at 0.
                        let resp =
                            format!(r#"{{"jsonrpc":"2.0","result":{sub_id},"id":{sub_id}}}"#);
                        ws.send(Message::Text(resp.into())).await.unwrap();
                    }
                });
            }
        });

        let addr = addr_rx.recv().unwrap();
        let ws_url = format!("ws://{}", addr);

        let client = super::Client::new(
            super::Cluster::Custom(ws_url.clone(), ws_url),
            std::sync::Arc::new(solana_keypair::Keypair::new()),
        );
        let program = client.program(Pubkey::new_unique()).unwrap();

        // With the old RwLock-based code, the second call would deadlock.
        // Use a timeout to ensure the test fails instead of hanging forever.
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            #[cfg(not(feature = "async"))]
            {
                let _listener1 = program
                    .on::<MockEvent>(|_ctx, _event| {})
                    .expect("first listener");

                let _listener2 = program
                    .on::<MockEvent>(|_ctx, _event| {})
                    .expect("second listener");
            }

            #[cfg(feature = "async")]
            {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async {
                    let _listener1 = program
                        .on::<MockEvent>(|_ctx, _event| {})
                        .await
                        .expect("first listener");

                    let _listener2 = program
                        .on::<MockEvent>(|_ctx, _event| {})
                        .await
                        .expect("second listener");
                });
            }

            let _ = done_tx.send(());
        });

        // If this times out, the deadlock is still present.
        done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("registering two listeners should not deadlock");

        handle.join().unwrap();
    }
}
