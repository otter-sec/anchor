use {
    anchor_lang_v2::{
        accounts::{Signer, UncheckedAccount},
        cpi::rent_exempt_lamports,
        testing::AccountBuffer,
        AccountInitialize, Accounts, AnchorAccount, Error, ErrorCode, Id, TryAccounts,
    },
    core::ops::Deref,
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const FOREIGN_OWNER: [u8; 32] = [0x24; 32];
const FAKE_MINT_LEN: usize = 66;
const FAKE_TOKEN_LEN: usize = 64;

#[derive(Accounts)]
struct SeedlessReuseUnchecked {
    #[account(init_if_needed, payer = payer, space = 8)]
    target: UncheckedAccount,
    #[account(mut)]
    payer: Signer,
}

#[derive(Clone, Copy)]
struct FakeMintData {
    authority: Address,
    decimals: u8,
    freeze_authority: Option<Address>,
}

struct FakeMintAccount {
    account: AccountView,
    data: FakeMintData,
}

impl FakeMintAccount {
    fn authority(&self) -> &Address {
        &self.data.authority
    }

    fn decimals(&self) -> u8 {
        self.data.decimals
    }

    fn freeze_authority(&self) -> Option<&Address> {
        self.data.freeze_authority.as_ref()
    }
}

impl Deref for FakeMintAccount {
    type Target = FakeMintData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl AnchorAccount for FakeMintAccount {
    type Data = FakeMintData;
    const MIN_DATA_LEN: usize = FAKE_MINT_LEN;

    fn load(view: AccountView) -> Result<Self, ProgramError> {
        let data = view.try_borrow()?;
        if data.len() < FAKE_MINT_LEN {
            return Err(ErrorCode::ConstraintSpace.into());
        }

        let mut authority = [0u8; 32];
        authority.copy_from_slice(&data[..32]);
        let decimals = data[32];
        let freeze_authority = if data[33] == 0 {
            None
        } else {
            let mut freeze = [0u8; 32];
            freeze.copy_from_slice(&data[34..66]);
            Some(Address::new_from_array(freeze))
        };

        Ok(Self {
            account: view,
            data: FakeMintData {
                authority: Address::new_from_array(authority),
                decimals,
                freeze_authority,
            },
        })
    }

    unsafe fn load_mut(view: AccountView) -> Result<Self, ProgramError> {
        if !view.is_writable() {
            return Err(ErrorCode::ConstraintMut.into());
        }
        Self::load(view)
    }

    fn account(&self) -> &AccountView {
        &self.account
    }
}

#[derive(Clone, Copy)]
struct FakeTokenData {
    mint: Address,
    authority: Address,
}

struct FakeTokenAccount {
    account: AccountView,
    data: FakeTokenData,
}

impl FakeTokenAccount {
    fn mint(&self) -> &Address {
        &self.data.mint
    }

    fn authority(&self) -> &Address {
        &self.data.authority
    }
}

impl Deref for FakeTokenAccount {
    type Target = FakeTokenData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl AnchorAccount for FakeTokenAccount {
    type Data = FakeTokenData;
    const MIN_DATA_LEN: usize = FAKE_TOKEN_LEN;

    fn load(view: AccountView) -> Result<Self, ProgramError> {
        let data = view.try_borrow()?;
        if data.len() < FAKE_TOKEN_LEN {
            return Err(ErrorCode::ConstraintSpace.into());
        }

        let mut mint = [0u8; 32];
        mint.copy_from_slice(&data[..32]);
        let mut authority = [0u8; 32];
        authority.copy_from_slice(&data[32..64]);

        Ok(Self {
            account: view,
            data: FakeTokenData {
                mint: Address::new_from_array(mint),
                authority: Address::new_from_array(authority),
            },
        })
    }

    unsafe fn load_mut(view: AccountView) -> Result<Self, ProgramError> {
        if !view.is_writable() {
            return Err(ErrorCode::ConstraintMut.into());
        }
        Self::load(view)
    }

    fn account(&self) -> &AccountView {
        &self.account
    }
}

#[derive(Default)]
struct FakeMintInitParams<'a> {
    decimals: Option<u8>,
    authority: Option<&'a AccountView>,
    freeze_authority: Option<&'a AccountView>,
}

impl AccountInitialize for FakeMintAccount {
    type Params<'a> = FakeMintInitParams<'a>;

    fn create_and_initialize<'a>(
        _payer: &AccountView,
        _account: &AccountView,
        _space: usize,
        _owner: &Address,
        params: &Self::Params<'a>,
        _signer_seeds: Option<&[&[u8]]>,
        _payer_signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        let _ = (params.decimals, params.authority, params.freeze_authority);
        Err(Error::InvalidAccountData)
    }
}

#[derive(Default)]
struct FakeTokenInitParams<'a> {
    mint: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
}

impl AccountInitialize for FakeTokenAccount {
    type Params<'a> = FakeTokenInitParams<'a>;

    fn create_and_initialize<'a>(
        _payer: &AccountView,
        _account: &AccountView,
        _space: usize,
        _owner: &Address,
        params: &Self::Params<'a>,
        _signer_seeds: Option<&[&[u8]]>,
        _payer_signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        let _ = (params.mint, params.authority);
        Err(Error::InvalidAccountData)
    }
}

mod mint {
    use {
        super::FakeMintAccount,
        anchor_lang_v2::{AccountConstraint, Error},
        solana_program_error::ProgramError,
    };

    pub struct AuthorityConstraint;

    impl AccountConstraint<FakeMintAccount> for AuthorityConstraint {
        type Value = pinocchio::address::Address;

        fn check(account: &FakeMintAccount, expected: &Self::Value) -> Result<(), ProgramError> {
            if !anchor_lang_v2::address_eq(account.authority(), expected) {
                return Err(Error::InvalidAccountData);
            }
            Ok(())
        }
    }

    pub struct DecimalsConstraint;

    impl AccountConstraint<FakeMintAccount> for DecimalsConstraint {
        type Value = u8;

        fn check(account: &FakeMintAccount, expected: &Self::Value) -> Result<(), ProgramError> {
            if account.decimals() != *expected {
                return Err(Error::InvalidAccountData);
            }
            Ok(())
        }
    }

    pub struct FreezeAuthorityConstraint;

    impl AccountConstraint<FakeMintAccount> for FreezeAuthorityConstraint {
        type Value = pinocchio::address::Address;

        fn check(account: &FakeMintAccount, expected: &Self::Value) -> Result<(), ProgramError> {
            if account.freeze_authority() != Some(expected) {
                return Err(Error::InvalidAccountData);
            }
            Ok(())
        }
    }
}

mod token {
    use {
        super::FakeTokenAccount,
        anchor_lang_v2::{AccountConstraint, Error},
        solana_program_error::ProgramError,
    };

    pub struct AuthorityConstraint;

    impl AccountConstraint<FakeTokenAccount> for AuthorityConstraint {
        type Value = pinocchio::address::Address;

        fn check(account: &FakeTokenAccount, expected: &Self::Value) -> Result<(), ProgramError> {
            if !anchor_lang_v2::address_eq(account.authority(), expected) {
                return Err(Error::InvalidAccountData);
            }
            Ok(())
        }
    }

    pub struct MintConstraint;

    impl AccountConstraint<FakeTokenAccount> for MintConstraint {
        type Value = pinocchio::address::Address;

        fn check(account: &FakeTokenAccount, expected: &Self::Value) -> Result<(), ProgramError> {
            if !anchor_lang_v2::address_eq(account.mint(), expected) {
                return Err(Error::InvalidAccountData);
            }
            Ok(())
        }
    }
}

#[derive(Accounts)]
struct ReuseFakeMint {
    #[account(
        init_if_needed,
        payer = payer,
        space = FAKE_MINT_LEN,
        mint::authority = authority,
        mint::decimals = 6
    )]
    target: FakeMintAccount,
    authority: UncheckedAccount,
    #[account(mut)]
    payer: Signer,
}

#[derive(Accounts)]
struct ReuseFakeMintWithFreezeAuthority {
    #[account(
        init_if_needed,
        payer = payer,
        space = FAKE_MINT_LEN,
        mint::authority = authority,
        mint::decimals = 6,
        mint::freeze_authority = freeze_authority
    )]
    target: FakeMintAccount,
    authority: UncheckedAccount,
    freeze_authority: UncheckedAccount,
    #[account(mut)]
    payer: Signer,
}

#[derive(Accounts)]
struct ReuseFakeToken {
    #[account(
        init_if_needed,
        payer = payer,
        space = FAKE_TOKEN_LEN,
        token::mint = mint,
        token::authority = authority
    )]
    target: FakeTokenAccount,
    mint: UncheckedAccount,
    authority: UncheckedAccount,
    #[account(mut)]
    payer: Signer,
}

fn expect_err<T>(result: Result<T, ProgramError>) -> ProgramError {
    match result {
        Ok(_) => panic!("expected Err, got Ok"),
        Err(err) => err,
    }
}

fn target_account(
    owner: [u8; 32],
    data_len: usize,
    signer: bool,
    lamports: u64,
) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init([0xAA; 32], owner, data_len, signer, true, false);
    buf.set_lamports(lamports);
    buf
}

fn payer_account() -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init([0xBB; 32], PROGRAM_ID, 0, true, true, false);
    buf
}

fn authority_account(address: [u8; 32]) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init(address, PROGRAM_ID, 0, false, false, false);
    buf
}

fn fake_mint_account(
    owner: [u8; 32],
    authority: [u8; 32],
    decimals: u8,
    freeze_authority: Option<[u8; 32]>,
    signer: bool,
) -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    buf.init([0xCC; 32], owner, FAKE_MINT_LEN, signer, true, false);
    let mut data = [0u8; FAKE_MINT_LEN];
    data[..32].copy_from_slice(&authority);
    data[32] = decimals;
    data[33] = u8::from(freeze_authority.is_some());
    if let Some(freeze_authority) = freeze_authority {
        data[34..66].copy_from_slice(&freeze_authority);
    }
    buf.write_data(&data);
    buf
}

fn fake_token_account(
    owner: [u8; 32],
    mint: [u8; 32],
    authority: [u8; 32],
    signer: bool,
) -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    buf.init([0xCD; 32], owner, FAKE_TOKEN_LEN, signer, true, false);
    let mut data = [0u8; FAKE_TOKEN_LEN];
    data[..32].copy_from_slice(&mint);
    data[32..64].copy_from_slice(&authority);
    buf.write_data(&data);
    buf
}

fn try_reuse(target: &AccountBuffer<128>, payer: &AccountBuffer<128>) -> Result<(), ProgramError> {
    let views = [unsafe { target.view() }, unsafe { payer.view() }];
    SeedlessReuseUnchecked::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, None, 0, &[])
        .map(|_| ())
}

fn try_reuse_fake_mint(
    target: &AccountBuffer<256>,
    authority: &AccountBuffer<128>,
    payer: &AccountBuffer<128>,
) -> Result<(), ProgramError> {
    let views = [
        unsafe { target.view() },
        unsafe { authority.view() },
        unsafe { payer.view() },
    ];
    ReuseFakeMint::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, None, 0, &[])
        .map(|_| ())
}

fn try_reuse_fake_mint_with_freeze_authority(
    target: &AccountBuffer<256>,
    authority: &AccountBuffer<128>,
    freeze_authority: &AccountBuffer<128>,
    payer: &AccountBuffer<128>,
) -> Result<(), ProgramError> {
    let views = [
        unsafe { target.view() },
        unsafe { authority.view() },
        unsafe { freeze_authority.view() },
        unsafe { payer.view() },
    ];
    ReuseFakeMintWithFreezeAuthority::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        None,
        0,
        &[],
    )
    .map(|_| ())
}

fn try_reuse_fake_token(
    target: &AccountBuffer<256>,
    mint: &AccountBuffer<128>,
    authority: &AccountBuffer<128>,
    payer: &AccountBuffer<128>,
) -> Result<(), ProgramError> {
    let views = [
        unsafe { target.view() },
        unsafe { mint.view() },
        unsafe { authority.view() },
        unsafe { payer.view() },
    ];
    ReuseFakeToken::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, None, 0, &[])
        .map(|_| ())
}

#[test]
fn seedless_reuse_requires_target_signature() {
    let target = target_account(PROGRAM_ID, 8, false, 1_000_000);
    let payer = payer_account();

    let err = expect_err(try_reuse(&target, &payer));
    assert_eq!(err, ErrorCode::ConstraintSigner.into());
}

#[test]
fn seedless_reuse_treats_zero_length_program_owned_target_as_existing() {
    let target = target_account(PROGRAM_ID, 0, true, 1_000_000);
    let payer = payer_account();

    let err = expect_err(try_reuse(&target, &payer));
    assert_eq!(err, ErrorCode::ConstraintSpace.into());
}

#[test]
fn seedless_reuse_rejects_extra_space() {
    let target = target_account(PROGRAM_ID, 16, true, 1_000_000);
    let payer = payer_account();

    let err = expect_err(try_reuse(&target, &payer));
    assert_eq!(err, ErrorCode::ConstraintSpace.into());
}

#[test]
fn seedless_reuse_revalidates_owner_for_unchecked_accounts() {
    let target = target_account(FOREIGN_OWNER, 8, true, 1_000_000);
    let payer = payer_account();

    let err = expect_err(try_reuse(&target, &payer));
    assert_eq!(err, ErrorCode::ConstraintOwner.into());
}

#[test]
fn seedless_reuse_revalidates_rent_exemption() {
    let target = target_account(PROGRAM_ID, 8, true, 1);
    let payer = payer_account();

    let err = expect_err(try_reuse(&target, &payer));
    assert_eq!(err, ErrorCode::ConstraintRentExempt.into());
}

#[test]
fn seedless_reuse_accepts_fully_initialized_accounts() {
    let required = rent_exempt_lamports(8).unwrap();
    let target = target_account(PROGRAM_ID, 8, true, required);
    let payer = payer_account();

    try_reuse(&target, &payer).expect("fully initialized reuse path should succeed");
}

#[test]
fn mint_reuse_requires_target_signature() {
    let authority = authority_account([0x77; 32]);
    let payer = payer_account();
    let target = fake_mint_account(PROGRAM_ID, [0x77; 32], 6, None, false);

    let err = expect_err(try_reuse_fake_mint(&target, &authority, &payer));
    assert_eq!(err, ErrorCode::ConstraintSigner.into());
}

#[test]
fn mint_reuse_with_explicit_freeze_authority_accepts_matching_reuse() {
    let authority = authority_account([0x90; 32]);
    let freeze_authority = authority_account([0x91; 32]);
    let payer = payer_account();
    let target = fake_mint_account(PROGRAM_ID, [0x90; 32], 6, Some([0x91; 32]), true);

    try_reuse_fake_mint_with_freeze_authority(&target, &authority, &freeze_authority, &payer)
        .expect("explicit freeze authority should suppress the omitted-freeze guard");
}

#[test]
fn mint_reuse_without_freeze_authority_attr_requires_none() {
    let authority = authority_account([0x88; 32]);
    let payer = payer_account();
    let target = fake_mint_account(PROGRAM_ID, [0x88; 32], 6, Some([0x99; 32]), true);

    let err = expect_err(try_reuse_fake_mint(&target, &authority, &payer));
    assert_eq!(err, Error::InvalidAccountData);
}

#[test]
fn token_reuse_requires_target_signature() {
    let mint = authority_account([0xA0; 32]);
    let authority = authority_account([0xA1; 32]);
    let payer = payer_account();
    let target = fake_token_account(PROGRAM_ID, [0xA0; 32], [0xA1; 32], false);

    let err = expect_err(try_reuse_fake_token(&target, &mint, &authority, &payer));
    assert_eq!(err, ErrorCode::ConstraintSigner.into());
}

#[test]
fn token_reuse_accepts_signed_target_when_constraints_match() {
    let mint = authority_account([0xB0; 32]);
    let authority = authority_account([0xB1; 32]);
    let payer = payer_account();
    let target = fake_token_account(PROGRAM_ID, [0xB0; 32], [0xB1; 32], true);

    try_reuse_fake_token(&target, &mint, &authority, &payer)
        .expect("signed token reuse should pass once token constraints match");
}
