#[doc(hidden)]
#[macro_export]
macro_rules! __anchor_log_instruction {
    ($name:expr) => {
        $crate::prelude::msg!($name)
    };
}
