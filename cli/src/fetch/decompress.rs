use {super::MAX_IDL_BUFFER_BYTES, flate2::read::ZlibDecoder, std::io::Read};

const ZLIB_HEADER: u8 = 0x78;

// Walks concatenated zlib streams and keeps only those that decode into valid IDL JSON.
pub(super) fn decompress_all_streams(compressed_data: &[u8]) -> Vec<Vec<u8>> {
    let mut streams = Vec::new();
    let mut cursor = compressed_data;

    while cursor.first() == Some(&ZLIB_HEADER) {
        let mut decoder = ZlibDecoder::new(cursor);
        let mut out = Vec::new();
        // Read one byte past the cap so that hitting `MAX + 1` definitively signals overflow
        // rather than a payload that legitimately ends on the boundary.
        let read_result = decoder
            .by_ref()
            .take(MAX_IDL_BUFFER_BYTES as u64 + 1)
            .read_to_end(&mut out);
        match read_result {
            Ok(_) => {
                if out.len() > MAX_IDL_BUFFER_BYTES {
                    // Stream exceeded the safety bound; stop walking because we no longer have
                    // a trustworthy boundary to resume from.
                    break;
                }
                let consumed = decoder.total_in() as usize;
                if is_complete_idl_json(&out) {
                    streams.push(out);
                }
                if consumed == 0 || consumed > cursor.len() {
                    break;
                }
                cursor = &cursor[consumed..];
            }
            Err(_) => break,
        }
    }

    streams
}

// Filters out truncated streams by requiring the decompressed payload to parse as JSON.
fn is_complete_idl_json(data: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(data).is_ok()
}
