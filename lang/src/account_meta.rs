use {crate::ToAccountMetas, solana_instruction::AccountMeta};

impl ToAccountMetas for AccountMeta {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<AccountMeta> {
        vec![self.clone()]
    }
}
