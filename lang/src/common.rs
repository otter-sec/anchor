use {
    crate::{
        error::{Error, ErrorCode},
        prelude::{Id, System},
        solana_program::{account_info::AccountInfo, pubkey::Pubkey, system_program},
        Lamports, Result,
    },
    std::io::{self, Write},
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

/// Exit path for an account that is no longer owned by the program, e.g.
/// after being reassigned via CPI. Mirrors the runtime: unchanged data is
/// tolerated, a modification is an error.
#[doc(hidden)]
pub fn exit_unowned<F>(info: &AccountInfo, program_id: &Pubkey, serialize: F) -> Result<()>
where
    F: FnOnce(&mut CompareWriter<'_>) -> Result<()>,
{
    let data = info.try_borrow_data()?;
    let mut writer = CompareWriter::new(&data);
    serialize(&mut writer)?;
    if writer.matches() {
        Ok(())
    } else {
        Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
            .with_pubkeys((*info.owner, *program_id)))
    }
}

/// `Write` sink that checks written bytes against an existing buffer
/// instead of storing them.
pub struct CompareWriter<'a> {
    data: &'a [u8],
    pos: usize,
    matches: bool,
}

impl<'a> CompareWriter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            matches: true,
        }
    }

    pub fn matches(&self) -> bool {
        self.matches
    }
}

impl Write for CompareWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let end = self.pos.saturating_add(buf.len());
        self.matches &= self.data.get(self.pos..end) == Some(buf);
        self.pos = end;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
