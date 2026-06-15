use {
    crate::{
        AsSigner, ClientError, Config, EventContext, EventUnsubscriber, Program,
        ProgramAccountsIterator, RequestBuilder, TxVersion,
    },
    anchor_lang::{prelude::Pubkey, AccountDeserialize, Discriminator},
    solana_commitment_config::CommitmentConfig,
    solana_rpc_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient,
    solana_rpc_client_api::{config::RpcSendTransactionConfig, filter::RpcFilterType},
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::{marker::PhantomData, ops::Deref, sync::Arc},
    tokio::sync::OnceCell,
};

impl<'a> EventUnsubscriber<'a> {
    /// Unsubscribe gracefully.
    pub async fn unsubscribe(self) {
        self.unsubscribe_internal().await
    }
}

pub trait ThreadSafeSigner: Signer + Send + Sync + 'static {
    fn to_signer(&self) -> &dyn Signer;
}

impl<T: Signer + Send + Sync + 'static> ThreadSafeSigner for T {
    fn to_signer(&self) -> &dyn Signer {
        self
    }
}

impl AsSigner for Arc<dyn ThreadSafeSigner> {
    fn as_signer(&self) -> &dyn Signer {
        self.to_signer()
    }
}

impl<C: Deref<Target = impl Signer> + Clone> Program<C> {
    pub fn new(
        program_id: Pubkey,
        cfg: Config<C>,
        #[cfg(feature = "mock")] rpc_client: AsyncRpcClient,
    ) -> Result<Self, ClientError> {
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
        })
    }

    // We disable the `rpc` method for `mock` feature because otherwise we'd either have to
    // return a new `RpcClient` instance (which is different to the one used internally)
    // or require the user to pass another one in for blocking (since we use the non-blocking one under the hood).
    // The former of these would be confusing and the latter would be very annoying, especially since a user
    // using the mock feature likely already has a `RpcClient` instance at hand anyway.
    #[cfg(not(feature = "mock"))]
    pub fn rpc(&self) -> AsyncRpcClient {
        AsyncRpcClient::new_with_commitment(
            self.cfg.cluster.url().to_string(),
            self.cfg.options.unwrap_or_default(),
        )
    }

    /// Returns a threadsafe request builder
    pub fn request(&self) -> RequestBuilder<'_, C, Arc<dyn ThreadSafeSigner>> {
        RequestBuilder::from(
            self.program_id,
            self.cfg.cluster.url(),
            self.cfg.payer.clone(),
            self.cfg.options,
            &self.internal_rpc_client,
        )
    }

    /// Returns the account at the given address.
    pub async fn account<T: AccountDeserialize>(&self, address: Pubkey) -> Result<T, ClientError> {
        self.account_internal(address).await
    }

    /// Returns all program accounts of the given type matching the given filters
    pub async fn accounts<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<Vec<(Pubkey, T)>, ClientError> {
        self.accounts_lazy(filters).await?.collect()
    }

    /// Returns all program accounts of the given type matching the given filters as an iterator
    /// Deserialization is executed lazily
    pub async fn accounts_lazy<T: AccountDeserialize + Discriminator>(
        &self,
        filters: Vec<RpcFilterType>,
    ) -> Result<ProgramAccountsIterator<T>, ClientError> {
        self.accounts_lazy_internal(filters).await
    }

    /// Subscribe to program logs.
    ///
    /// Returns an [`EventUnsubscriber`] to unsubscribe and close connection gracefully.
    pub async fn on<T: anchor_lang::Event + anchor_lang::AnchorDeserialize>(
        &self,
        f: impl FnMut(&EventContext, T) + Send + 'static,
    ) -> Result<EventUnsubscriber<'_>, ClientError> {
        let (handle, rx) = self.on_internal(f).await?;

        Ok(EventUnsubscriber {
            handle,
            rx,
            _lifetime_marker: PhantomData,
        })
    }
}

impl<'a, C: Deref<Target = impl Signer> + Clone> RequestBuilder<'a, C, Arc<dyn ThreadSafeSigner>> {
    pub fn from(
        program_id: Pubkey,
        cluster: &str,
        payer: C,
        options: Option<CommitmentConfig>,
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
            internal_rpc_client: rpc_client,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    pub fn signer<T: ThreadSafeSigner>(mut self, signer: T) -> Self {
        self.signers.push(Arc::new(signer));
        self
    }

    /// Build and sign a transaction.
    ///
    /// Note: This will use a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`signed_transaction_versioned`].
    pub async fn signed_transaction(&self) -> Result<Transaction, ClientError> {
        self.signed_transaction_internal(TxVersion::Legacy)
            .await
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
    /// // Legacy transaction
    /// let tx = request.signed_transaction_versioned(TxVersion::Legacy).unwrap();
    ///
    /// // V0 transaction
    /// let tx = request.signed_transaction_versioned(TxVersion::V0(&[lookup_table])).unwrap();
    /// ```
    pub async fn signed_transaction_versioned(
        &self,
        version: TxVersion<'_>,
    ) -> Result<solana_transaction::versioned::VersionedTransaction, ClientError> {
        self.signed_transaction_internal(version).await
    }

    /// Send a transaction.
    ///
    /// Note: This will use a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`send_versioned`].
    pub async fn send(&self) -> Result<Signature, ClientError> {
        self.send_internal(TxVersion::Legacy).await
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
    pub async fn send_versioned(&self, version: TxVersion<'_>) -> Result<Signature, ClientError> {
        self.send_internal(version).await
    }

    /// Send a transaction with spinner and config.
    ///
    /// Note: This will use a transaction with the legacy transaction format. If you'd like to use
    /// a different transaction format, use [`send_with_spinner_and_config_versioned`].
    pub async fn send_with_spinner_and_config(
        &self,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        self.send_with_spinner_and_config_internal(TxVersion::Legacy, config)
            .await
    }

    /// Send a transaction with the specified version, spinner and config.
    ///
    /// # Arguments
    ///
    /// * `version` - The transaction version to use ([`TxVersion::Legacy`] or [`TxVersion::V0`]).
    /// * `config` - RPC send transaction configuration.
    pub async fn send_with_spinner_and_config_versioned(
        &self,
        version: TxVersion<'_>,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        self.send_with_spinner_and_config_internal(version, config)
            .await
    }
}
