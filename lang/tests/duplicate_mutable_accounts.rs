use anchor_lang::{prelude::*, Bumps};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
#[derive(Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Counter {
    count: u64,
}

impl anchor_lang::ZeroCopy for Counter {}

#[derive(Accounts)]
struct Direct<'info> {
    #[account(mut)]
    account: Account<'info, Counter>,
    #[account(mut)]
    loader: AccountLoader<'info, Counter>,
}

#[derive(Accounts)]
struct Loader<'info> {
    #[account(mut)]
    loader: Option<AccountLoader<'info, Counter>>,
}

#[derive(Accounts)]
struct Nested<'info> {
    #[account(mut)]
    account: Account<'info, Counter>,
    nested: Loader<'info>,
}

#[derive(Accounts)]
struct TwoLoaders<'info> {
    #[account(mut)]
    first: AccountLoader<'info, Counter>,
    #[account(mut)]
    second: AccountLoader<'info, Counter>,
}

#[derive(Accounts)]
struct NestedLoaders<'info> {
    first: Loader<'info>,
    second: Loader<'info>,
}

#[derive(Accounts)]
struct Serializing<'info> {
    #[account(mut)]
    account: Account<'info, Counter>,
}

#[derive(Accounts)]
struct MixedNested<'info> {
    loaders: NestedLoaders<'info>,
    serializing: Serializing<'info>,
}

#[derive(Accounts)]
struct AllowDuplicate<'info> {
    #[account(mut)]
    account: Account<'info, Counter>,
    #[account(mut, dup)]
    loader: AccountLoader<'info, Counter>,
}

fn validate<'info, A: Bumps + Accounts<'info, A::Bumps>>(
    mut accounts: &'info [AccountInfo<'info>],
) -> Result<()>
where
    A::Bumps: Default,
{
    A::try_accounts(
        &ID,
        &mut accounts,
        &[],
        &mut A::Bumps::default(),
        &mut Default::default(),
    )
    .map(drop)
}

#[test]
fn rejects_loader_aliases_with_serializing_accounts() {
    let key = Pubkey::new_unique();
    let other_key = Pubkey::new_unique();
    let mut data = Vec::new();
    Counter { count: 0 }.try_serialize(&mut data).unwrap();
    let mut other_data = data.clone();
    let mut lamports = 1;
    let mut other_lamports = 1;
    let account = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &ID, false);
    let other = AccountInfo::new(
        &other_key,
        false,
        true,
        &mut other_lamports,
        &mut other_data,
        &ID,
        false,
    );

    let duplicate = [account.clone(), account.clone()];
    let distinct = [account.clone(), other.clone()];
    let mixed_duplicate = [account.clone(), account.clone(), account.clone()];
    let mixed_distinct = [account.clone(), account, other];
    let expected = ErrorCode::ConstraintDuplicateMutableAccount.into();

    assert_eq!(validate::<Direct>(&duplicate).unwrap_err(), expected);
    assert_eq!(validate::<Nested>(&duplicate).unwrap_err(), expected);
    assert_eq!(
        validate::<MixedNested>(&mixed_duplicate).unwrap_err(),
        expected
    );
    validate::<Direct>(&distinct).unwrap();
    validate::<Nested>(&distinct).unwrap();
    validate::<AllowDuplicate>(&duplicate).unwrap();
    validate::<TwoLoaders>(&duplicate).unwrap();
    validate::<NestedLoaders>(&duplicate).unwrap();
    validate::<MixedNested>(&mixed_distinct).unwrap();
}
