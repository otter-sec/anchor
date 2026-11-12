// Avoiding AccountInfo deprecated msg in anchor context
#![allow(deprecated)]
use {
    crate::{
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, Result, ToAccountInfos, ToAccountMetas,
    },
    std::collections::BTreeSet,
};

impl<'info, T: ToAccountInfos<'info>> ToAccountInfos<'info> for Vec<T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        self.iter()
            .flat_map(|item| item.to_account_infos())
            .collect()
    }
}

impl<T: ToAccountMetas> ToAccountMetas for Vec<T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.iter()
            .flat_map(|item| (*item).to_account_metas(is_signer))
            .collect()
    }
}

impl<'info, B, T: Accounts<'info, B>> Accounts<'info, B> for Vec<T> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut B,
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let mut vec: Vec<T> = Vec::new();
        T::try_accounts(program_id, accounts, ix_data, bumps, reallocs)
            .map(|item| vec.push(item))?;
        Ok(vec)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{accounts::unchecked_account::UncheckedAccount, solana_program::pubkey::Pubkey},
    };

    #[derive(Accounts)]
    pub struct Test<'info> {
        #[account(signer)]
        test: UncheckedAccount<'info>,
    }

    #[test]
    fn test_accounts_trait_for_vec() {
        let program_id = Pubkey::default();

        let key = Pubkey::default();
        let mut lamports1 = 0;
        let mut data1 = vec![0; 10];
        let owner = Pubkey::default();
        let account1 =
            AccountInfo::new(&key, true, true, &mut lamports1, &mut data1, &owner, false);

        let mut lamports2 = 0;
        let mut data2 = vec![0; 10];
        let account2 =
            AccountInfo::new(&key, true, true, &mut lamports2, &mut data2, &owner, false);
        let mut bumps = TestBumps::default();
        let mut reallocs = std::collections::BTreeSet::new();
        let mut accounts = &[account1, account2][..];
        let parsed_accounts =
            Vec::<Test>::try_accounts(&program_id, &mut accounts, &[], &mut bumps, &mut reallocs)
                .unwrap();

        assert_eq!(accounts.len(), parsed_accounts.len());
    }

    #[test]
    #[should_panic]
    fn test_accounts_trait_for_vec_empty() {
        let program_id = Pubkey::default();
        let mut bumps = TestBumps::default();
        let mut reallocs = std::collections::BTreeSet::new();
        let mut accounts = &[][..];
        Vec::<Test>::try_accounts(&program_id, &mut accounts, &[], &mut bumps, &mut reallocs)
            .unwrap();
    }
}
