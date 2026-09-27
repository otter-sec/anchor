pub mod accounts;
pub mod context;
pub mod docs;
pub mod error;
pub mod program;

pub fn tts_to_string<T: quote::ToTokens>(item: T) -> String {
    item.to_token_stream().to_string()
}
