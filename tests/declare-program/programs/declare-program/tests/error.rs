#![cfg(not(feature = "idl-build"))]

use anchor_lang::prelude::*;

declare_program!(external);
use external::error::ExternalError;

#[test]
pub fn test_error_code() {
    assert_eq!(ExternalError::Default as u32, 6000);
    assert_eq!(ExternalError::WithOffset as u32, 6500);
    assert_eq!(ExternalError::WithMsg as u32, 6501);
}

#[test]
pub fn test_error_msg() {
    assert_eq!(ExternalError::Default.to_string().as_str(), "Default");
    assert_eq!(ExternalError::WithOffset.to_string().as_str(), "WithOffset");
    assert_eq!(ExternalError::WithMsg.to_string().as_str(), "Custom msg");
}
