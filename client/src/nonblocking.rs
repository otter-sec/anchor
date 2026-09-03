use {
    crate::{
        AsSigner, ClientError, Config, EventContext, EventUnsubscriber, Program,
        ProgramAccountsIterator, RequestBuilder,
    },
    anchor_lang::{AccountDeserialize, Discriminator},
    solana_commitment_config::CommitmentConfig,
    solana_program::pubkey::Pubkey,
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
    pub async fn on<
        T: anchor_lang::Event
            + for<'de> anchor_lang::wincode::SchemaRead<'de, anchor_lang::BorshConfig, Dst = T>,
    >(
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

    pub async fn signed_transaction(&self) -> Result<Transaction, ClientError> {
        self.signed_transaction_internal().await
    }

    pub async fn send(self) -> Result<Signature, ClientError> {
        self.send_internal().await
    }

    pub async fn send_with_spinner_and_config(
        self,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature, ClientError> {
        self.send_with_spinner_and_config_internal(config).await
    }
}
