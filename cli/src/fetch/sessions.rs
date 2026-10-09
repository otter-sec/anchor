use super::{SessionChunks, SlotChunk};

// Legacy uploads normally write their chunks in a tight slot range. A 5k-slot break is large
// enough to avoid splitting slow uploads while still separating unrelated historical sessions.
const SESSION_SLOT_GAP_THRESHOLD: u64 = 5_000;

// Groups ordered chunks into upload sessions using chunk-size transitions and slot gaps.
pub(super) fn group_chunks_into_sessions(all_chunks: &[SlotChunk]) -> Vec<SessionChunks> {
    let mut sessions: Vec<SessionChunks> = Vec::new();
    let mut current: SessionChunks = Vec::new();
    let mut terminator_seen = false;

    for chunk in all_chunks {
        let size = chunk.2.len();
        let last = current.last();
        let prev_size = last.map(|(_, _, data)| data.len());
        let prev_slot = last.map(|(slot, _, _)| *slot);

        let slot_gap_break = matches!(
            prev_slot,
            Some(prev) if chunk.0.saturating_sub(prev) > SESSION_SLOT_GAP_THRESHOLD
        );

        let start_new = slot_gap_break
            || match prev_size {
                Some(prev) => terminator_seen || size > prev,
                None => false,
            };

        if start_new {
            sessions.push(std::mem::take(&mut current));
            terminator_seen = false;
        }

        if let Some(prev) = prev_size {
            if !start_new && size < prev {
                terminator_seen = true;
            }
        }

        current.push(chunk.clone());
    }

    if !current.is_empty() {
        sessions.push(current);
    }

    sessions
}
