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

/// Finds the first key that repeats an earlier one, for the duplicate mutable
/// account check. `None` entries (absent optional accounts) never match.
///
/// Quadratic, but a compare is ~10 CU and the keys before the first repeat are
/// distinct writable accounts, so their count is capped by the transaction's
/// account lock limit. Sorting would only win past ~64 keys and costs far more
/// program size.
#[doc(hidden)]
pub fn find_duplicate_key<'a>(keys: &[Option<&'a Pubkey>]) -> Option<(usize, &'a Pubkey)> {
    keys.iter().enumerate().find_map(|(i, key)| {
        let key = (*key)?;
        keys[..i].contains(&Some(key)).then_some((i, key))
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: u8) -> Vec<Pubkey> {
        (0..n).map(|i| Pubkey::new_from_array([i; 32])).collect()
    }

    #[test]
    fn find_duplicate_key_matches_insert_order() {
        let k = keys(8);
        let mut refs: Vec<Option<&Pubkey>> = k.iter().map(Some).collect();
        assert_eq!(find_duplicate_key(&refs), None);

        // Absent optional accounts never collide with each other.
        refs[1] = None;
        refs[3] = None;
        assert_eq!(find_duplicate_key(&refs), None);

        // Two collisions: the one whose later key comes first is reported.
        refs[6] = Some(&k[2]);
        refs[5] = Some(&k[4]);
        refs[7] = Some(&k[2]);
        assert_eq!(find_duplicate_key(&refs), Some((5, &k[4])));
    }
}
