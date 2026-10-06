//! Tests for `SerializedAccount<T, S>` with non-borsh codecs.
//!
//! The wrapper is generic over `S: AnchorAccountSerialize<T>`, so users can
//! plug any encoding. These tests exercise:
//!
//! 1. A hand-rolled fixed-size little-endian codec (`LeCodec`) — represents
//!    a user with no schema library at all, just raw bytes.
//! 2. A Wincode-backed codec (`WincodeCodec`) — uses Anchor's derives and
//!    public Wincode re-export without a direct Wincode dependency.
//!
//! Each codec is exercised through the same lifecycle as `BorshAccount<T>`:
//! `load`, `load_mut` + mutate + `exit`, `release_borrow` + `reacquire_borrow_mut`,
//! and the load-time validation failure paths (wrong owner, wrong disc,
//! short data).
//!
//! The init path (`AccountInitialize::create_and_initialize`) is not covered
//! here — it issues a real CPI that host tests cannot satisfy.

use {
    anchor_lang::{
        accounts::{AnchorAccountSerialize, SerializedAccount},
        testing::AccountBuffer,
        wincode, AnchorAccount, AnchorDeserialize, AnchorSerialize, Discriminator, Owner,
    },
    pinocchio::{account::RuntimeAccount, address::Address},
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];

// -- shared test helpers --------------------------------------------------

fn read_data_bytes<const N: usize>(buf: &AccountBuffer<N>, offset: usize, len: usize) -> Vec<u8> {
    let header = core::mem::size_of::<RuntimeAccount>();
    let start = header + offset;
    unsafe {
        let base = buf as *const AccountBuffer<N> as *const u8;
        core::slice::from_raw_parts(base.add(start), len).to_vec()
    }
}

fn set_data_bytes<const N: usize>(buf: &mut AccountBuffer<N>, offset: usize, bytes: &[u8]) {
    let header = core::mem::size_of::<RuntimeAccount>();
    let start = header + offset;
    unsafe {
        let base = buf.raw() as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(start), bytes.len());
    }
}

// =========================================================================
// Codec 1: hand-rolled fixed-size little-endian (no schema library).
// =========================================================================

#[derive(Default, Clone, PartialEq, Debug)]
struct Stats {
    count: u32,
    flags: u32,
}

const STATS_DISC: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

impl Owner for Stats {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Stats {
    const DISCRIMINATOR: &'static [u8] = &STATS_DISC;
}

/// User-defined codec. Fixed 8-byte LE layout, no length prefix.
struct LeCodec;

impl AnchorAccountSerialize<Stats> for LeCodec {
    fn serialize(value: &Stats, buf: &mut &mut [u8]) -> Result<(), ProgramError> {
        if buf.len() < 8 {
            return Err(ProgramError::InvalidAccountData);
        }
        let tmp = core::mem::take(buf);
        let (payload, rest) = tmp.split_at_mut(8);
        payload[..4].copy_from_slice(&value.count.to_le_bytes());
        payload[4..8].copy_from_slice(&value.flags.to_le_bytes());
        *buf = rest;
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Stats, ProgramError> {
        if buf.len() < 8 {
            return Err(ProgramError::InvalidAccountData);
        }
        let (payload, rest) = buf.split_at(8);
        *buf = rest;
        Ok(Stats {
            count: u32::from_le_bytes(payload[..4].try_into().unwrap()),
            flags: u32::from_le_bytes(payload[4..8].try_into().unwrap()),
        })
    }
}

type StatsAccount = SerializedAccount<Stats, LeCodec>;

fn setup_stats_buf(buf: &mut AccountBuffer<256>, count: u32, flags: u32) {
    let data_len = 8 + 8; // disc(8) + Stats(8)
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(&STATS_DISC);
    data[8..12].copy_from_slice(&count.to_le_bytes());
    data[12..16].copy_from_slice(&flags.to_le_bytes());
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

// -- 1.1 immutable load deserializes through the custom codec -------------

#[test]
fn le_codec_immutable_load_deserializes() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 7, 0xABCD_1234);

    let view = unsafe { buf.view() };
    let acct = StatsAccount::load(view).unwrap();
    assert_eq!(acct.count, 7);
    assert_eq!(acct.flags, 0xABCD_1234);
}

// -- 1.2 mutate + exit writes through the custom codec --------------------

#[test]
fn le_codec_load_mut_exit_writes_back() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { StatsAccount::load_mut(view) }.unwrap();
        acct.count = 42;
        acct.flags = 0xDEAD_BEEF;
        acct.exit().unwrap();
    }

    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 42);
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        0xDEAD_BEEF
    );
}

// -- 1.3 release_borrow commits in-memory state --------------------------

#[test]
fn le_codec_release_borrow_commits() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { StatsAccount::load_mut(view) }.unwrap();
    acct.count = 999;
    acct.release_borrow().unwrap();

    let bytes = read_data_bytes(&buf, 8, 4);
    assert_eq!(
        u32::from_le_bytes(bytes.try_into().unwrap()),
        999,
        "release_borrow must serialize via the user codec before dropping the guard"
    );
}

// -- 1.4 reacquire refreshes self.data through the custom codec ----------

#[test]
fn le_codec_reacquire_refreshes_from_buffer() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { StatsAccount::load_mut(view) }.unwrap();

    acct.count = 100;
    acct.release_borrow().unwrap();

    // Simulated CPI: rewrites the entire payload in the wire format.
    let mut new_payload = [0u8; 8];
    new_payload[..4].copy_from_slice(&777u32.to_le_bytes());
    new_payload[4..8].copy_from_slice(&0x1111_2222u32.to_le_bytes());
    set_data_bytes(&mut buf, 8, &new_payload);

    acct.reacquire_borrow_mut().unwrap();
    assert_eq!(acct.count, 777);
    assert_eq!(acct.flags, 0x1111_2222);
}

#[test]
#[should_panic(
    expected = "SerializedAccount mutated through a read-only load. Add #[account(mut)] to your \
                accounts struct."
)]
fn le_codec_reacquire_mut_panics_when_loaded_read_only() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);

    let view = unsafe { buf.view() };
    let mut acct = StatsAccount::load(view).unwrap();
    acct.release_borrow().unwrap();
    acct.reacquire_borrow_mut().unwrap();
}

// -- 1.5 load fails on wrong discriminator -------------------------------

#[test]
fn le_codec_load_rejects_wrong_discriminator() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);
    // Corrupt the disc.
    set_data_bytes(&mut buf, 0, &[0u8; 8]);

    let view = unsafe { buf.view() };
    let err = StatsAccount::load(view).err();
    assert_eq!(err, Some(ProgramError::InvalidAccountData));
}

// -- 1.6 load fails on wrong owner ---------------------------------------

#[test]
fn le_codec_load_rejects_wrong_owner() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);
    buf.set_owner([0xFE; 32]);

    let view = unsafe { buf.view() };
    let err = StatsAccount::load(view).err();
    assert_eq!(err, Some(ProgramError::IllegalOwner));
}

// -- 1.7 load fails on short data ----------------------------------------

#[test]
fn le_codec_load_rejects_short_data() {
    let buf = AccountBuffer::<256>::new();
    // Disc-length only, no payload.
    buf.init([0xAA; 32], PROGRAM_ID, 8, false, true, false);
    buf.write_data(&STATS_DISC);
    buf.set_lamports(1_000_000_000);

    let view = unsafe { buf.view() };
    // The codec returns InvalidAccountData when the payload is too short
    // (8 - 8 = 0 bytes available, codec needs 8).
    assert_eq!(
        StatsAccount::load(view).err(),
        Some(ProgramError::InvalidAccountData)
    );
}

// -- 1.8 reacquire rejects when discriminator changes during release -----

#[test]
fn le_codec_reacquire_rejects_disc_swap() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { StatsAccount::load_mut(view) }.unwrap();
    acct.release_borrow().unwrap();

    set_data_bytes(&mut buf, 0, &[0xFF; 8]);
    let err = acct.reacquire_borrow_mut().unwrap_err();
    assert_eq!(err, ProgramError::InvalidAccountData);
}

// -- 1.9 reacquire rejects when owner changes during release ------------

#[test]
fn le_codec_reacquire_rejects_owner_change() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { StatsAccount::load_mut(view) }.unwrap();
    acct.release_borrow().unwrap();

    buf.set_owner([0xFE; 32]);
    let err = acct.reacquire_borrow_mut().unwrap_err();
    assert_eq!(err, ProgramError::IllegalOwner);
}

// -- 1.10 exit on transient zero-lamport account serializes --------------

#[test]
fn le_codec_exit_on_transient_zero_lamport_account_serializes() {
    let mut buf = AccountBuffer::<256>::new();
    setup_stats_buf(&mut buf, 1, 0xAABB_CCDD);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { StatsAccount::load_mut(view) }.unwrap();
    acct.count = 555;
    buf.set_lamports(0);
    acct.exit().unwrap();

    // Zero lamports alone do not prove the account was closed. Persist the
    // update unless the closed-account discriminator is also present.
    let bytes = read_data_bytes(&buf, 8, 4);
    assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), 555);
}

// =========================================================================
// Codec 2: Wincode through Anchor's public re-export.
// =========================================================================
//
// Demonstrates that `AnchorAccountSerialize<T>` can be implemented by
// delegating to an external schema framework. We use the same
// borsh-compatible wire config that v2 uses for events / instruction args.

#[derive(Default, Clone, PartialEq, Debug, AnchorDeserialize, AnchorSerialize)]
struct Ledger {
    balance: u64,
    nonce: u32,
}

const LEDGER_DISC: [u8; 8] = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];

impl Owner for Ledger {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Ledger {
    const DISCRIMINATOR: &'static [u8] = &LEDGER_DISC;
}

/// Wincode-backed codec. Bounds restrict `T` to schemas that round-trip
/// through themselves (`Src = T`, `Dst = T`), matching the contract exposed
/// by Anchor's `wincode::serialize` / `wincode::deserialize` re-export.
struct WincodeCodec;

impl<T> AnchorAccountSerialize<T> for WincodeCodec
where
    T: wincode::Serialize<Src = T> + wincode::DeserializeOwned<Dst = T>,
{
    fn serialize(value: &T, buf: &mut &mut [u8]) -> Result<(), ProgramError> {
        let bytes = wincode::serialize(value).map_err(|_| ProgramError::InvalidAccountData)?;
        if buf.len() < bytes.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let tmp = core::mem::take(buf);
        let (payload, rest) = tmp.split_at_mut(bytes.len());
        payload.copy_from_slice(&bytes);
        *buf = rest;
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<T, ProgramError> {
        let value: T = wincode::deserialize(*buf).map_err(|_| ProgramError::InvalidAccountData)?;
        let bytes = wincode::serialize(&value).map_err(|_| ProgramError::InvalidAccountData)?;
        if buf.len() < bytes.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        *buf = &buf[bytes.len()..];
        Ok(value)
    }
}

type LedgerAccount = SerializedAccount<Ledger, WincodeCodec>;

fn setup_ledger_buf(buf: &mut AccountBuffer<256>, balance: u64, nonce: u32) {
    let payload = wincode::serialize(&Ledger { balance, nonce }).unwrap();
    let data_len = 8 + payload.len();
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = Vec::with_capacity(data_len);
    data.extend_from_slice(&LEDGER_DISC);
    data.extend_from_slice(&payload);
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

#[test]
fn wincode_codec_load_deserializes() {
    let mut buf = AccountBuffer::<256>::new();
    setup_ledger_buf(&mut buf, 1_000_000, 42);

    let view = unsafe { buf.view() };
    let acct = LedgerAccount::load(view).unwrap();
    assert_eq!(acct.balance, 1_000_000);
    assert_eq!(acct.nonce, 42);
}

#[test]
fn wincode_codec_load_mut_exit_round_trip() {
    let mut buf = AccountBuffer::<256>::new();
    setup_ledger_buf(&mut buf, 0, 0);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { LedgerAccount::load_mut(view) }.unwrap();
        acct.balance = 9_999;
        acct.nonce = 7;
        acct.exit().unwrap();
    }

    // Re-load and verify the wire bytes deserialize back to the new values.
    let view = unsafe { buf.view() };
    let acct = LedgerAccount::load(view).unwrap();
    assert_eq!(acct.balance, 9_999);
    assert_eq!(acct.nonce, 7);
}

#[test]
fn wincode_codec_rejects_wrong_disc() {
    let mut buf = AccountBuffer::<256>::new();
    setup_ledger_buf(&mut buf, 0, 0);
    set_data_bytes(&mut buf, 0, &[0u8; 8]);

    let view = unsafe { buf.view() };
    assert_eq!(
        LedgerAccount::load(view).err(),
        Some(ProgramError::InvalidAccountData)
    );
}

#[test]
fn wincode_codec_release_reacquire_picks_up_cpi_write() {
    let mut buf = AccountBuffer::<256>::new();
    setup_ledger_buf(&mut buf, 0, 0);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { LedgerAccount::load_mut(view) }.unwrap();
    acct.release_borrow().unwrap();

    // Simulated CPI writes a new ledger payload using the same codec.
    let cpi_payload = wincode::serialize(&Ledger {
        balance: 12_345,
        nonce: 99,
    })
    .unwrap();
    set_data_bytes(&mut buf, 8, &cpi_payload);

    acct.reacquire_borrow_mut().unwrap();
    assert_eq!(acct.balance, 12_345);
    assert_eq!(acct.nonce, 99);
}

// =========================================================================
// Variable-length discriminators (Finding #103)
//
// `Discriminator` is a byte slice and `declare_program!` / imported accounts
// may use non-8-byte discs. SerializedAccount must size/load/serialize from
// `T::DISCRIMINATOR.len()`, not a hardcoded 8.
// =========================================================================

#[derive(Default, Clone, PartialEq, Debug)]
struct Compact {
    value: u32,
}

const COMPACT_DISC: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];

impl Owner for Compact {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Compact {
    const DISCRIMINATOR: &'static [u8] = &COMPACT_DISC;
}

impl AnchorAccountSerialize<Compact> for LeCodec {
    fn serialize(value: &Compact, buf: &mut &mut [u8]) -> Result<(), ProgramError> {
        if buf.len() < 4 {
            return Err(ProgramError::InvalidAccountData);
        }
        let tmp = core::mem::take(buf);
        let (payload, rest) = tmp.split_at_mut(4);
        payload.copy_from_slice(&value.value.to_le_bytes());
        *buf = rest;
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Compact, ProgramError> {
        if buf.len() < 4 {
            return Err(ProgramError::InvalidAccountData);
        }
        let (payload, rest) = buf.split_at(4);
        *buf = rest;
        Ok(Compact {
            value: u32::from_le_bytes(payload.try_into().unwrap()),
        })
    }
}

type CompactAccount = SerializedAccount<Compact, LeCodec>;

fn setup_compact_buf(buf: &mut AccountBuffer<256>, value: u32) {
    let data_len = COMPACT_DISC.len() + 4;
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 8];
    data[..4].copy_from_slice(&COMPACT_DISC);
    data[4..8].copy_from_slice(&value.to_le_bytes());
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

#[derive(Default, Clone, PartialEq, Debug)]
struct Wide {
    value: u32,
}

const WIDE_DISC: [u8; 16] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
];

impl Owner for Wide {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Wide {
    const DISCRIMINATOR: &'static [u8] = &WIDE_DISC;
}

impl AnchorAccountSerialize<Wide> for LeCodec {
    fn serialize(value: &Wide, buf: &mut &mut [u8]) -> Result<(), ProgramError> {
        if buf.len() < 4 {
            return Err(ProgramError::InvalidAccountData);
        }
        let tmp = core::mem::take(buf);
        let (payload, rest) = tmp.split_at_mut(4);
        payload.copy_from_slice(&value.value.to_le_bytes());
        *buf = rest;
        Ok(())
    }

    fn deserialize(buf: &mut &[u8]) -> Result<Wide, ProgramError> {
        if buf.len() < 4 {
            return Err(ProgramError::InvalidAccountData);
        }
        let (payload, rest) = buf.split_at(4);
        *buf = rest;
        Ok(Wide {
            value: u32::from_le_bytes(payload.try_into().unwrap()),
        })
    }
}

type WideAccount = SerializedAccount<Wide, LeCodec>;

fn setup_wide_buf(buf: &mut AccountBuffer<256>, value: u32) {
    let data_len = WIDE_DISC.len() + 4;
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 20];
    data[..16].copy_from_slice(&WIDE_DISC);
    data[16..20].copy_from_slice(&value.to_le_bytes());
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

#[test]
fn four_byte_discriminator_load_and_exit_round_trip() {
    assert_eq!(
        <CompactAccount as AnchorAccount>::MIN_DATA_LEN,
        COMPACT_DISC.len()
    );

    let mut buf = AccountBuffer::<256>::new();
    setup_compact_buf(&mut buf, 7);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { CompactAccount::load_mut(view) }.unwrap();
        assert_eq!(acct.value, 7);
        acct.value = 99;
        acct.exit().unwrap();
    }

    assert_eq!(read_data_bytes(&buf, 0, 4), COMPACT_DISC);
    let payload = read_data_bytes(&buf, 4, 4);
    assert_eq!(u32::from_le_bytes(payload.try_into().unwrap()), 99);
}

#[test]
fn sixteen_byte_discriminator_load_and_exit_round_trip() {
    assert_eq!(
        <WideAccount as AnchorAccount>::MIN_DATA_LEN,
        WIDE_DISC.len()
    );

    let mut buf = AccountBuffer::<256>::new();
    setup_wide_buf(&mut buf, 11);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { WideAccount::load_mut(view) }.unwrap();
        assert_eq!(acct.value, 11);
        acct.value = 55;
        acct.exit().unwrap();
    }

    assert_eq!(read_data_bytes(&buf, 0, 16), WIDE_DISC);
    let payload = read_data_bytes(&buf, 16, 4);
    assert_eq!(u32::from_le_bytes(payload.try_into().unwrap()), 55);
}

#[test]
fn four_byte_discriminator_rejects_buffer_shorter_than_disc() {
    let mut buf = AccountBuffer::<256>::new();
    // Eight-byte buffer would have passed the old hardcoded DISC_LEN=8 floor
    // for a four-byte disc type, but still be short of a real 8-byte payload
    // layout. A three-byte buffer is unambiguously below Compact's disc.
    buf.init([0xAA; 32], PROGRAM_ID, 3, false, true, false);
    buf.write_data(&[0xde, 0xad, 0xbe]);
    buf.set_lamports(1_000_000_000);

    let view = unsafe { buf.view() };
    let err = CompactAccount::load(view).err();
    assert_eq!(err, Some(ProgramError::AccountDataTooSmall));
}

#[test]
fn sixteen_byte_discriminator_rejects_classic_eight_byte_prefix() {
    // Pre-fix SerializedAccount would slice only eight bytes and compare
    // them to a sixteen-byte DISCRIMINATOR (length mismatch → reject). Post-
    // fix it still rejects an eight-byte buffer as too small for a 16-byte
    // disc — the important case is that a correctly laid-out 16+payload
    // account loads (covered above).
    let mut buf = AccountBuffer::<256>::new();
    buf.init([0xAA; 32], PROGRAM_ID, 8, false, true, false);
    buf.write_data(&WIDE_DISC[..8]);
    buf.set_lamports(1_000_000_000);

    let view = unsafe { buf.view() };
    let err = WideAccount::load(view).err();
    assert_eq!(err, Some(ProgramError::AccountDataTooSmall));
}
