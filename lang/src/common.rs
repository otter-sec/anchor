use crate::{
    prelude::{Id, System},
    solana_program::{account_info::AccountInfo, system_program},
    Lamports, Result,
};

pub(crate) fn close<'info>(
    info: &AccountInfo<'info>,
    sol_destination: &AccountInfo<'info>,
) -> Result<()> {
    // Transfer lamports from the account to the sol_destination.
    sol_destination.add_lamports(info.lamports())?;
    **info.lamports.borrow_mut() = 0;

    info.assign(&system_program::ID);
    info.resize(0).map_err(Into::into)
}

pub fn is_closed(info: &AccountInfo) -> bool {
    info.owner == &System::id() && info.data_is_empty()
}
