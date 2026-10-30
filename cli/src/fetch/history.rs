use {
    sha2::{Digest, Sha256},
    std::collections::HashSet,
};

// Identifies which on-chain storage path produced a recovered historical IDL so merge and output
// logic can preserve provenance when legacy and PMP histories overlap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum IdlHistorySource {
    Legacy,
    Pmp,
}

impl IdlHistorySource {
    // Converts the source into the filename suffix used when two recovered versions share a slot.
    pub fn as_suffix(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Pmp => "pmp",
        }
    }
}

// Normalizes legacy chunk-session recoveries and PMP metadata recoveries into one common record so
// the rest of historical fetch can sort, deduplicate, and write them uniformly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalIdlVersion {
    pub slot: u64,
    pub signature: String,
    pub source: IdlHistorySource,
    pub idl_data: Vec<u8>,
}

// Merges independently reconstructed legacy and PMP histories into one newest-first timeline while
// collapsing identical same-slot payloads down to a single preferred entry.
pub fn merge_historical_idls(
    legacy: Vec<HistoricalIdlVersion>,
    pmp: Vec<HistoricalIdlVersion>,
) -> Vec<HistoricalIdlVersion> {
    let mut candidates: Vec<_> = legacy.into_iter().chain(pmp).collect();
    // Prefer newer slots first and sort PMP entries ahead of legacy ones so same-slot duplicate
    // payloads collapse to the PMP source during deduplication.
    candidates.sort_by(|a, b| {
        b.slot
            .cmp(&a.slot)
            .then_with(|| source_rank(b.source).cmp(&source_rank(a.source)))
            .then_with(|| a.signature.cmp(&b.signature))
    });

    let mut merged = Vec::new();
    // Deduplicate by slot plus payload hash so identical legacy/PMP recoveries collapse without
    // cloning the full IDL bytes into the seen set.
    let mut seen: HashSet<(u64, [u8; 32])> = HashSet::new();

    for item in candidates {
        let key = (item.slot, payload_hash(&item.idl_data));
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        merged.push(item);
    }
    merged
}

// Ranks sources for merge precedence so PMP wins when a legacy and PMP recovery produce the same
// payload at the same slot.
fn source_rank(source: IdlHistorySource) -> u8 {
    match source {
        IdlHistorySource::Legacy => 0,
        IdlHistorySource::Pmp => 1,
    }
}

// Hashes the payload bytes for in-memory deduplication without storing another full IDL copy in
// the seen set.
fn payload_hash(bytes: &[u8]) -> [u8; 32] {
    // Use a stable digest so deduplication is deterministic across runs and does not depend on
    // randomized hash seeds.
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_prefers_pmp_for_duplicate_slot_and_payload() {
        let legacy = HistoricalIdlVersion {
            slot: 10,
            signature: "legacy".into(),
            source: IdlHistorySource::Legacy,
            idl_data: b"{}".to_vec(),
        };
        let pmp = HistoricalIdlVersion {
            slot: 10,
            signature: "pmp".into(),
            source: IdlHistorySource::Pmp,
            idl_data: b"{}".to_vec(),
        };

        let merged = merge_historical_idls(vec![legacy], vec![pmp.clone()]);
        assert_eq!(merged, vec![pmp]);
    }
}
