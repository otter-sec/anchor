//! Account composition shared by individual wrappers and derived account groups.

use {
    crate::{
        AccountBitvec, AccountMeta, AccountView, Address, AnchorAccount, CpiHandle, CpiHandleMut,
        Result,
    },
    alloc::vec::Vec,
};

/// A field in an `Accounts` struct. Account groups implement this through
/// `#[derive(Accounts)]`, so nesting requires no marker attribute or wrapper.
#[doc(hidden)]
pub trait AccountField: crate::Bumps<Bumps: Default + Clone> + Sized {
    const HEADER_SIZE: usize;
    const MUT_MASK: [u64; 4] = [0; 4];
    const HAS_DYNAMIC_MUT_MASK: bool = false;
    const IS_SIGNER: bool = false;
    type Target;
    fn constraint_target(&self) -> Option<&Self::Target>;
    type Client;
    type Cpi<'a>;
    type CpiMut<'a>;

    fn load(
        program_id: &Address,
        views: &[AccountView],
        duplicates: Option<&AccountBitvec>,
        base_offset: usize,
        ix_data: &[u8],
    ) -> Result<(Self, Self::Bumps)>;
    fn active_mut_mask(&self) -> [u64; 4] {
        Self::MUT_MASK
    }
    fn update(&mut self) -> Result<()> {
        Ok(())
    }
    fn exit(&mut self, _ix_data: &[u8]) -> Result<()> {
        Ok(())
    }
    fn append_client_metas(
        client: &Self::Client,
        program_id: &Address,
        writable: bool,
        signer: bool,
        signer_override: Option<bool>,
        out: &mut Vec<AccountMeta>,
    );
}

impl<T: AnchorAccount> AccountField for T {
    const HEADER_SIZE: usize = 1;
    const IS_SIGNER: bool = T::IS_SIGNER;
    type Target = T;
    fn constraint_target(&self) -> Option<&T> {
        Some(self)
    }
    type Client = Address;
    type Cpi<'a> = CpiHandle<'a>;
    type CpiMut<'a> = CpiHandleMut<'a>;
    #[inline(always)]
    fn load(
        _: &Address,
        views: &[AccountView],
        _: Option<&AccountBitvec>,
        _: usize,
        _: &[u8],
    ) -> Result<(Self, ())> {
        Ok((T::load(views[0])?, ()))
    }
    #[inline(always)]
    fn append_client_metas(
        client: &Address,
        _: &Address,
        writable: bool,
        signer: bool,
        signer_override: Option<bool>,
        out: &mut Vec<AccountMeta>,
    ) {
        let _ = signer_override;
        out.push(AccountMeta {
            pubkey: *client,
            is_writable: writable,
            is_signer: signer,
        });
    }
}

impl<T: AnchorAccount> AccountField for Option<T> {
    const HEADER_SIZE: usize = 1;
    const IS_SIGNER: bool = T::IS_SIGNER;
    type Target = T;
    fn constraint_target(&self) -> Option<&T> {
        self.as_ref()
    }
    type Client = Option<Address>;
    type Cpi<'a> = Option<CpiHandle<'a>>;
    type CpiMut<'a> = Option<CpiHandleMut<'a>>;
    #[inline(always)]
    fn load(
        program_id: &Address,
        views: &[AccountView],
        _: Option<&AccountBitvec>,
        _: usize,
        _: &[u8],
    ) -> Result<(Self, ())> {
        let view = views[0];
        let account = if crate::address_eq(view.address(), program_id) {
            None
        } else {
            Some(T::load(view)?)
        };
        Ok((account, ()))
    }
    #[inline(always)]
    fn append_client_metas(
        client: &Option<Address>,
        program_id: &Address,
        writable: bool,
        signer: bool,
        signer_override: Option<bool>,
        out: &mut Vec<AccountMeta>,
    ) {
        match client {
            Some(address) => {
                T::append_client_metas(address, program_id, writable, signer, signer_override, out)
            }
            None => out.push(AccountMeta {
                pubkey: *program_id,
                is_writable: false,
                is_signer: false,
            }),
        }
    }
}

/// One account slot, either required or optional. Constraints dispatch on
/// `Account`, after Rust has resolved aliases and qualified paths.
#[doc(hidden)]
pub trait AccountSlot: AccountField {
    type Account: AnchorAccount;
    type PdaBump: Default + Clone;
    const IS_OPTIONAL: bool;
    fn cache_bump(bump: u8) -> Self::PdaBump;
    fn load_with(
        view: AccountView,
        program_id: &Address,
        load: impl FnOnce(AccountView) -> Result<Self::Account>,
    ) -> Result<Self>;
    fn as_account(&self) -> Option<&Self::Account>;
    #[inline(always)]
    fn require_account(&self) -> Result<&Self::Account> {
        self.as_account()
            .ok_or_else(|| crate::ErrorCode::ConstraintAccountIsNone.into())
    }
    fn as_account_mut(&mut self) -> Option<&mut Self::Account>;
}

impl<T: AnchorAccount> AccountSlot for T {
    type Account = T;
    type PdaBump = u8;
    const IS_OPTIONAL: bool = false;
    fn cache_bump(bump: u8) -> u8 {
        bump
    }
    #[inline(always)]
    fn load_with(
        view: AccountView,
        _: &Address,
        load: impl FnOnce(AccountView) -> Result<T>,
    ) -> Result<Self> {
        load(view)
    }
    #[inline(always)]
    fn as_account(&self) -> Option<&T> {
        Some(self)
    }
    #[inline(always)]
    fn as_account_mut(&mut self) -> Option<&mut T> {
        Some(self)
    }
}

impl<T: AnchorAccount> AccountSlot for Option<T> {
    type Account = T;
    type PdaBump = Option<u8>;
    const IS_OPTIONAL: bool = true;
    fn cache_bump(bump: u8) -> Option<u8> {
        Some(bump)
    }
    #[inline(always)]
    fn load_with(
        view: AccountView,
        program_id: &Address,
        load: impl FnOnce(AccountView) -> Result<T>,
    ) -> Result<Self> {
        if crate::address_eq(view.address(), program_id) {
            Ok(None)
        } else {
            load(view).map(Some)
        }
    }
    #[inline(always)]
    fn as_account(&self) -> Option<&T> {
        self.as_ref()
    }
    #[inline(always)]
    fn as_account_mut(&mut self) -> Option<&mut T> {
        self.as_mut()
    }
}

/// A system-owned account suitable for paying for PDA initialization.
#[doc(hidden)]
pub trait PdaPayer: AnchorAccount {}
impl PdaPayer for crate::accounts::SystemAccount {}
impl<T: PdaPayer> PdaPayer for alloc::boxed::Box<T> {}
