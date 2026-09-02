#[cfg(not(feature = "mock"))]
use solana_rpc_client::rpc_client::RpcClient;
use {
    crate::{
        ClientError, Config, EventContext, EventUnsubscriber, Program, ProgramAccountsIterator,
        RequestBuilder, TxVersion,
    },
    anchor_lang::{prelude::Pubkey, AccountDeserialize, Discriminator},
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient,
    solana_rpc_client_api::{config::RpcSendTransactionConfig, filter::RpcFilterType},
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::{marker::PhantomData, ops::Deref},
    tokio::{
        runtime::{Builder, Handle},
        sync::OnceCell,
    },
};

impl EventUnsubscriber<'_> {
    /// Unsubscribe gracefully.
    pub fn unsubscribe(self) {
        self.runtime_handle.block_on(self.unsubscribe_internal())
    }
}

impl<C: Deref<Target = impl Signer> + Clone> Program<C> {
    pub fn new(
        program_id: Pubkey,
        cfg: Config<C>,
        #[cfg(feature = "mock")] rpc_client: AsyncRpcClient,
    ) -> Result<Self, ClientError> {
        let rt: tokio::runtime::Runtime = Builder::new_multi_thread().enable_all().build()?;

        #[cfg(not(feature = "mock"))]
        let rpc_client = {
            let comm_config = cfg.options.unwrap_or_default();
            let cluster_url = cfg.cluster.url().to_string();
            AsyncRpcClient::new_with_commitment(cluster_url.clone(), comm_config)
        };

        Ok(Self {
            program_id,
            cfg,
            sub_client: OnceCell::new(),
            internal_rpc_client: rpc_client,
            rt,
        })
    }

    // We disable the `rpc` method for `mock` feature because otherwise we'd either have to
    // return a new `RpcClient` instance (which is different to the one used internally)
    // or require the user to pass another one in for blocking (since we use the non-blocking one under the hood).
    // The former of these would be confusing and the latter would be very annoying, especially since a user
    // using the mock feature likely already has a `RpcClient` instance at hand anyway.
    #[cfg(not(feature = "mock"))]
    pub fn rpc(&self) -> RpcClient {
        RpcClient::new_with_commitment(
            self.cfg.cluster.url().to_string(),
            self.cfg.options.unwrap_or_default(),
        )
    }

    /// Returns a request builder.
    pub fn request(&self) -> RequestBuilder<'_, C, Box<dyn Signer + '_>> {
        RequestBuilder::from(
            self.program_id,
            self.cfg.cluster.url(),
            self.cfg.payer.clone(),
            self.cfg.options,
            #[cfg(not(feature = "async"))]
            self.rt.handle(),
            &self.internal_rpc_client,
        )
    }

    /// Returns the account at the given address.
    pub fn account<T: AccountDeserialize>(&self, address: Pubkey) -> Result<T, ClientError> {
        self.rt.block_on(self.account_internal(address))
    }

    /// Returns all program accounts of the given type matching the given filters
    pub fn accounts<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<Vec<(Pubkey, T)>, ClientError> {
        self.accounts_lazy(filters)?.collect()
    }

    /// Returns all program accounts of the given type matching the given filters as an iterator
    /// Deserialization is executed lazily
    pub fn accounts_lazy<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<ProgramAccountsIterator<T>, ClientError> {
        self.rt.block_on(self.accounts_lazy_internal(filters))
    }

    pub fn on<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
        &self,
        f: impl FnMut(&EventContext, T) + Send + 'static,
    ) -> Result<EventUnsubscriber<'_>, ClientError> {
        let (handle, rx) = self.rt.block_on(self.on_internal(f))?;

        Ok(EventUnsubscriber {
            handle,
            rx,
            runtime_handle: self.rt.handle(),
            _lifetime_marker: PhantomData,
        })
    }
}

impl<'a, C: Deref<Target = impl Signer> + Clone> RequestBuilder<'a, C, Box<dyn Signer + 'a>> {
    pub fn from(
        program_id: Pubkey,
        cluster: &str,
        payer: C,
        options: Option<CommitmentConfig>,
        handle: &'a Handle,
        rpc_client: &'a AsyncRpcClient,
    ) -> Self {
        Self {
            program_id,
            payer,
            cluster: cluster.to_string(),
            accounts: Vec::new(),
            options: options.unwrap_or_default(),
            instructions: Vec::new(),
            instruction_data: None,
            signers: Vec::new(),
            handle,
            internal_rpc_client: rpc_client,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    pub fn signer<T: Signer + 'a>(mut self, signer: T) -> Self {
        self.signers.push(Box::new(signer));
        self
    }

    /// Build and sign a transaction.
    ///
    /// Note: This will use a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`signed_transaction_versioned`].
    pub fn signed_transaction(&self) -> Result<Transaction, ClientError> {
        self.handle
            .block_on(self.signed_transaction_internal(TxVersion::Legacy))
            .map(|tx| {
                tx.into_legacy_transaction()
                    .expect("Signed transaction with `TxVersion::Legacy`")
            })
    }

    /// Sign and return a transaction with the specified version.
    ///
    /// # Arguments
    ///
    /// * `version` - The transaction version to use ([`TxVersion::Legacy`] or [`TxVersion::V0`]).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anchor_client::{Client, Cluster, TxVersion};
    /// use anchor_lang::prelude::Pubkey;
    /// use solana_signer::null_signer::NullSigner;
    /// use solana_message::AddressLookupTableAccount;
    ///
    /// let payer = NullSigner::new(&Pubkey::default());
    /// let client = Client::new(Cluster::Localnet, std::rc::Rc::new(payer));
    ///
    /// let program = client.program(Pubkey::default()).unwrap();
    /// let lookup_table = AddressLookupTableAccount { key: Pubkey::default(), addresses: vec![] };
    /// let request = program.request();
    /// // Legacy transaction
    /// let tx = request.signed_transaction_versioned(TxVersion::Legacy).unwrap();
    ///
    /// // V0 transaction
    /// let tx = request.signed_transaction_versioned(TxVersion::V0(&[lookup_table])).unwrap();
    /// ```
    pub fn signed_transaction_versioned(
        &self,
        version: TxVersion<'_>,
    ) -> Result<solana_transaction::versioned::VersionedTransaction, ClientError> {
        self.handle
            .block_on(self.signed_transaction_internal(version))
    }

    /// Send a transaction.
    ///
    /// Note: This will use a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`send_versioned`].
    pub fn send(&self) -> Result<Signature, ClientError> {
        self.handle.block_on(self.send_internal(TxVersion::Legacy))
    }

    /// Send a transaction with the specified version.
    ///
    /// # Arguments
    ///
    /// * `version` - The transaction version to use ([`TxVersion::Legacy`] or [`TxVersion::V0`]).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anchor_client::{Client, Cluster, TxVersion};
    /// use anchor_lang::prelude::Pubkey;
    /// use solana_signer::null_signer::NullSigner;
    /// use solana_message::AddressLookupTableAccount;
    ///
    /// let payer = NullSigner::new(&Pubkey::default());
    /// let client = Client::new(Cluster::Localnet, std::rc::Rc::new(payer));
    ///
    /// let program = client.program(Pubkey::default()).unwrap();
    /// let lookup_table = AddressLookupTableAccount { key: Pubkey::default(), addresses: vec![] };
    ///
    /// let request = program.request();
    /// // Legacy transaction
    /// let sig = request.send_versioned(TxVersion::Legacy).unwrap();
    ///
    /// // V0 transaction with lookup tables
    /// let sig = request.send_versioned(TxVersion::V0(&[lookup_table])).unwrap();
    /// ```
    pub fn send_versioned(&self, version: TxVersion<'_>) -> Result<Signature, ClientError> {
        self.handle.block_on(self.send_internal(version))
    }

    /// Send a transaction with spinner and config.
    ///
    /// Note: This will use a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`send_with_spinner_and_config_versioned`].
    pub fn send_with_spinner_and_config(
        &self,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        self.handle
            .block_on(self.send_with_spinner_and_config_internal(TxVersion::Legacy, config))
    }

    /// Send a transaction with the specified version, spinner and config.
    ///
    /// # Arguments
    ///
    /// * `version` - The transaction version to use ([`TxVersion::Legacy`] or [`TxVersion::V0`]).
    /// * `config` - RPC send transaction configuration.
    pub fn send_with_spinner_and_config_versioned(
        &self,
        version: TxVersion<'_>,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        self.handle
            .block_on(self.send_with_spinner_and_config_internal(version, config))
    }
}
