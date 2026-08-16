//! Program to perform instantly settled token swaps on the Serum DEX.
//!
//! Before using any instruction here, a user must first create an open orders
//! account on all markets being used. This only needs to be done once. As a
//! convention established by the DEX, this should be done via the system
//! program create account instruction in the same transaction as the user's
//! first trade. Then, the DEX will lazily initialize the open orders account.

use anchor_lang::prelude::*;
use anchor_spl::dex;
use anchor_spl::token;
use serum_dex::instruction::SelfTradeBehavior;
use serum_dex::matching::{OrderType, Side as SerumSide};
use serum_dex::state::MarketState;
use std::num::NonZeroU64;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod swap {
    use super::*;

    /// Swaps two tokens on a single A/B market, where A is the base currency
    /// and B is the quote currency. This is just a direct IOC trade that
    /// instantly settles.
    ///
    /// When side is "bid", then swaps B for A. When side is "ask", then swaps
    /// A for B.
    ///
    /// Arguments:
    ///
    /// * `side`                     - The direction to swap.
    /// * `amount`                   - The amount to swap *from*
    /// * `min_expected_swap_amount` - The minimum amount of the *to* token the
    ///    client expects to receive from the swap. The instruction fails if
    ///    execution would result in less.
    #[access_control(is_valid_swap(&ctx))]
    pub fn swap(
        ctx: Context<Swap>,
        side: Side,
        amount: u64,
        min_expected_swap_amount: u64,
    ) -> Result<()> {
        // Optional referral account (earns a referral fee).
        let referral = ctx.remaining_accounts.iter().next().map(Clone::clone);

        // Side determines swap direction.
        let (from_token, to_token) = match side {
            Side::Bid => (&ctx.accounts.pc_wallet, &ctx.accounts.market.coin_wallet),
            Side::Ask => (&ctx.accounts.market.coin_wallet, &ctx.accounts.pc_wallet),
        };

        // Token balances before the trade.
        let from_amount_before = token::accessor::amount(from_token)?;
        let to_amount_before = token::accessor::amount(to_token)?;

        // Execute trade.
        let orderbook: OrderbookClient<'info> = (&*ctx.accounts).into();
        match side {
            Side::Bid => orderbook.buy(amount, referral.clone())?,
            Side::Ask => orderbook.sell(amount, referral.clone())?,
        };
        orderbook.settle(referral)?;

        // Token balances after the trade.
        let from_amount_after = token::accessor::amount(from_token)?;
        let to_amount_after = token::accessor::amount(to_token)?;

        //  Calculate the delta, i.e. the amount swapped.
        let from_amount = from_amount_before.checked_sub(from_amount_after).unwrap();
        let to_amount = to_amount_after.checked_sub(to_amount_before).unwrap();

        // Safety checks.
        apply_risk_checks(DidSwap {
            authority: *ctx.accounts.authority.key,
            given_amount: amount,
            min_expected_swap_amount,
            from_amount,
            to_amount,
            spill_amount: 0,
            from_mint: token::accessor::mint(from_token)?,
            to_mint: token::accessor::mint(to_token)?,
            quote_mint: match side {
                Side::Bid => token::accessor::mint(from_token)?,
                Side::Ask => token::accessor::mint(to_token)?,
            },
        })?;

        Ok(())
    }

    /// Swaps two base currencies across two different markets.
    ///
    /// That is, suppose there are two markets, A/USD(x) and B/USD(x).
    /// Then swaps token A for token B via
    ///
    /// * IOC (immediate or cancel) sell order on A/USD(x) market.
    /// * Settle open orders to get USD(x).
    /// * IOC buy order on B/USD(x) market to convert USD(x) to token B.
    /// * Settle open orders to get token B.
    ///
    /// Arguments:
    ///
    /// * `amount`                  - The amount to swap *from*.
    /// * `min_expected_swap_amount - The minimum amount of the *to* token the
    ///    client expects to receive from the swap. The instruction fails if
    ///    execution would result in less.
    #[access_control(is_valid_swap_transitive(&ctx))]
    pub fn swap_transitive(
        ctx: Context<SwapTransitive>,
        amount: u64,
        min_expected_swap_amount: u64,
    ) -> Result<()> {
        // Optional referral account (earns a referral fee).
        let referral = ctx.remaining_accounts.iter().next().map(Clone::clone);

        // Leg 1: Sell Token A for USD(x) (or whatever quote currency is used).
        let (from_amount, sell_proceeds) = {
            // Token balances before the trade.
            let base_before = token::accessor::amount(&ctx.accounts.from.coin_wallet)?;
            let quote_before = token::accessor::amount(&ctx.accounts.pc_wallet)?;

            // Execute the trade.
            let orderbook = ctx.accounts.orderbook_from();
            orderbook.sell(amount, referral.clone())?;
            orderbook.settle(referral.clone())?;

            // Token balances after the trade.
            let base_after = token::accessor::amount(&ctx.accounts.from.coin_wallet)?;
            let quote_after = token::accessor::amount(&ctx.accounts.pc_wallet)?;

            // Report the delta.
            (
                base_before.checked_sub(base_after).unwrap(),
                quote_after.checked_sub(quote_before).unwrap(),
            )
        };

        // Leg 2: Buy Token B with USD(x) (or whatever quote currency is used).
        let (to_amount, spill_amount) = {
            // Token balances before the trade.
            let base_before = token::accessor::amount(&ctx.accounts.to.coin_wallet)?;
            let quote_before = token::accessor::amount(&ctx.accounts.pc_wallet)?;

            // Execute the trade.
            let orderbook = ctx.accounts.orderbook_to();
            orderbook.buy(sell_proceeds, referral.clone())?;
            orderbook.settle(referral)?;

            // Token balances after the trade.
            let base_after = token::accessor::amount(&ctx.accounts.to.coin_wallet)?;
            let quote_after = token::accessor::amount(&ctx.accounts.pc_wallet)?;

            // Report the delta.
            (
                base_after.checked_sub(base_before).unwrap(),
                quote_before.checked_sub(quote_after).unwrap(),
            )
        };

        // Safety checks.
        apply_risk_checks(DidSwap {
            given_amount: amount,
            min_expected_swap_amount,
            from_amount,
            to_amount,
            spill_amount,
            from_mint: token::accessor::mint(&ctx.accounts.from.coin_wallet)?,
            to_mint: token::accessor::mint(&ctx.accounts.to.coin_wallet)?,
            quote_mint: token::accessor::mint(&ctx.accounts.pc_wallet)?,
            authority: *ctx.accounts.authority.key,
        })?;

        Ok(())
    }
}

// Asserts the swap event is valid.
fn apply_risk_checks(event: DidSwap) -> Result<()> {
    // Reject if the resulting amount is less than the client's expectation.
    if event.to_amount < event.min_expected_swap_amount {
        return err!(ErrorCode::SlippageExceeded);
    }
    emit!(event);
    Ok(())
}

// The only constraint imposed on these accounts is that the market's base
// currency mint is not equal to the quote currency's. All other checks are
// done by the DEX on CPI.
#[derive(Accounts)]
pub struct Swap<'info> {
    market: MarketAccounts<'info>,
    #[account(signer)]
    authority: UncheckedAccount<'info>,
    #[account(mut)]
    pc_wallet: UncheckedAccount<'info>,
    // Programs.
    dex_program: UncheckedAccount<'info>,
    token_program: UncheckedAccount<'info>,
    // Sysvars.
    rent: UncheckedAccount<'info>,
}

impl<'info> From<&Swap<'info>> for OrderbookClient<'info> {
    fn from(accounts: &Swap<'info>) -> OrderbookClient<'info> {
        OrderbookClient {
            market: accounts.market.clone(),
            authority: accounts.authority.clone(),
            pc_wallet: accounts.pc_wallet.clone(),
            dex_program: accounts.dex_program.clone(),
            token_program: accounts.token_program.clone(),
            rent: accounts.rent.clone(),
        }
    }
}

// The only constraint imposed on these accounts is that the from market's
// base currency's is not equal to the to market's base currency. All other
// checks are done by the DEX on CPI (and the quote currency is ensured to be
// the same on both markets since there's only one account field for it).
#[derive(Accounts)]
pub struct SwapTransitive<'info> {
    from: MarketAccounts<'info>,
    to: MarketAccounts<'info>,
    // Must be the authority over all open orders accounts used.
    #[account(signer)]
    authority: UncheckedAccount<'info>,
    #[account(mut)]
    pc_wallet: UncheckedAccount<'info>,
    // Programs.
    dex_program: UncheckedAccount<'info>,
    token_program: UncheckedAccount<'info>,
    // Sysvars.
    rent: UncheckedAccount<'info>,
}

impl<'info> SwapTransitive<'info> {
    fn orderbook_from(&self) -> OrderbookClient<'info> {
        OrderbookClient {
            market: self.from.clone(),
            authority: self.authority.clone(),
            pc_wallet: self.pc_wallet.clone(),
            dex_program: self.dex_program.clone(),
            token_program: self.token_program.clone(),
            rent: self.rent.clone(),
        }
    }
    fn orderbook_to(&self) -> OrderbookClient<'info> {
        OrderbookClient {
            market: self.to.clone(),
            authority: self.authority.clone(),
            pc_wallet: self.pc_wallet.clone(),
            dex_program: self.dex_program.clone(),
            token_program: self.token_program.clone(),
            rent: self.rent.clone(),
        }
    }
}

// Client for sending orders to the Serum DEX.
struct OrderbookClient<'info> {
    market: MarketAccounts<'info>,
    authority: UncheckedAccount<'info>,
    pc_wallet: UncheckedAccount<'info>,
    dex_program: UncheckedAccount<'info>,
    token_program: UncheckedAccount<'info>,
    rent: UncheckedAccount<'info>,
}

impl<'info> OrderbookClient<'info> {
    // Executes the sell order portion of the swap, purchasing as much of the
    // quote currency as possible for the given `base_amount`.
    //
    // `base_amount` is the "native" amount of the base currency, i.e., token
    // amount including decimals.
    fn sell(&self, base_amount: u64, referral: Option<AccountInfo<'info>>) -> Result<()> {
        let limit_price = 1;
        let max_coin_qty = {
            // The loaded market must be dropped before CPI.
            let market = MarketState::load(&self.market.market, &dex::ID)
                .map_err(|de| ProgramError::from(de))?;
            coin_lots(&market, base_amount)
        };
        let max_native_pc_qty = u64::MAX;
        self.order_cpi(
            limit_price,
            max_coin_qty,
            max_native_pc_qty,
            Side::Ask,
            referral,
        )
    }

    // Executes the buy order portion of the swap, purchasing as much of the
    // base currency as possible, for the given `quote_amount`.
    //
    // `quote_amount` is the "native" amount of the quote currency, i.e., token
    // amount including decimals.
    fn buy(&self, quote_amount: u64, referral: Option<AccountInfo<'info>>) -> Result<()> {
        let limit_price = u64::MAX;
        let max_coin_qty = u64::MAX;
        let max_native_pc_qty = quote_amount;
        self.order_cpi(
            limit_price,
            max_coin_qty,
            max_native_pc_qty,
            Side::Bid,
            referral,
        )
    }

    // Executes a new order on the serum dex via CPI.
    //
    // * `limit_price` - the limit order price in lot units.
    // * `max_coin_qty`- the max number of the base currency lot units.
    // * `max_native_pc_qty` - the max number of quote currency in native token
    //                         units (includes decimals).
    // * `side` - bid or ask, i.e. the type of order.
    // * `referral` - referral account, earning a fee.
    fn order_cpi(
        &self,
        limit_price: u64,
        max_coin_qty: u64,
        max_native_pc_qty: u64,
        side: Side,
        referral: Option<AccountInfo<'info>>,
    ) -> Result<()> {
        // Client order id is only used for cancels. Not used here so hardcode.
        let client_order_id = 0;
        // Limit is the dex's custom compute budge parameter, setting an upper
        // bound on the number of matching cycles the program can perform
        // before giving up and posting the remaining unmatched order.
        let limit = 65535;

        let dex_accs = dex::NewOrderV3 {
            market: self.market.market.clone(),
            open_orders: self.market.open_orders.clone(),
            request_queue: self.market.request_queue.clone(),
            event_queue: self.market.event_queue.clone(),
            market_bids: self.market.bids.clone(),
            market_asks: self.market.asks.clone(),
            order_payer_token_account: self.market.order_payer_token_account.clone(),
            open_orders_authority: self.authority.clone(),
            coin_vault: self.market.coin_vault.clone(),
            pc_vault: self.market.pc_vault.clone(),
            token_program: self.token_program.clone(),
            rent: self.rent.clone(),
        };
        let mut ctx = CpiContext::new(dex_accs);
        if let Some(referral) = referral {
            ctx = ctx.with_remaining_accounts(vec![referral]);
        }
        dex::new_order_v3(
            ctx,
            side.into(),
            NonZeroU64::new(limit_price).unwrap(),
            NonZeroU64::new(max_coin_qty).unwrap(),
            NonZeroU64::new(max_native_pc_qty).unwrap(),
            SelfTradeBehavior::DecrementTake,
            OrderType::ImmediateOrCancel,
            client_order_id,
            limit,
        )
    }

    fn settle(&self, referral: Option<AccountInfo<'info>>) -> Result<()> {
        let settle_accs = dex::SettleFunds {
            market: self.market.market.clone(),
            open_orders: self.market.open_orders.clone(),
            open_orders_authority: self.authority.clone(),
            coin_vault: self.market.coin_vault.clone(),
            pc_vault: self.market.pc_vault.clone(),
            coin_wallet: self.market.coin_wallet.clone(),
            pc_wallet: self.pc_wallet.clone(),
            vault_signer: self.market.vault_signer.clone(),
            token_program: self.token_program.clone(),
        };
        let mut ctx = CpiContext::new(settle_accs);
        if let Some(referral) = referral {
            ctx = ctx.with_remaining_accounts(vec![referral]);
        }
        dex::settle_funds(ctx)
    }
}

// Returns the amount of lots for the base currency of a trade with `size`.
fn coin_lots(market: &MarketState, size: u64) -> u64 {
    size.checked_div(market.coin_lot_size).unwrap()
}

// Market accounts are the accounts used to place orders against the dex minus
// common accounts, i.e., program ids, sysvars, and the `pc_wallet`.
#[derive(Accounts, Clone)]
pub struct MarketAccounts<'info> {
    #[account(mut)]
    market: UncheckedAccount<'info>,
    #[account(mut)]
    open_orders: UncheckedAccount<'info>,
    #[account(mut)]
    request_queue: UncheckedAccount<'info>,
    #[account(mut)]
    event_queue: UncheckedAccount<'info>,
    #[account(mut)]
    bids: UncheckedAccount<'info>,
    #[account(mut)]
    asks: UncheckedAccount<'info>,
    // The `spl_token::Account` that funds will be taken from, i.e., transferred
    // from the user into the market's vault.
    //
    // For bids, this is the base currency. For asks, the quote.
    #[account(mut)]
    order_payer_token_account: UncheckedAccount<'info>,
    // Also known as the "base" currency. For a given A/B market,
    // this is the vault for the A mint.
    #[account(mut)]
    coin_vault: UncheckedAccount<'info>,
    // Also known as the "quote" currency. For a given A/B market,
    // this is the vault for the B mint.
    #[account(mut)]
    pc_vault: UncheckedAccount<'info>,
    // PDA owner of the DEX's token accounts for base + quote currencies.
    vault_signer: UncheckedAccount<'info>,
    // User wallets.
    #[account(mut)]
    coin_wallet: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum Side {
    Bid,
    Ask,
}

impl From<Side> for SerumSide {
    fn from(side: Side) -> SerumSide {
        match side {
            Side::Bid => SerumSide::Bid,
            Side::Ask => SerumSide::Ask,
        }
    }
}

// Access control modifiers.

fn is_valid_swap(ctx: &Context<Swap>) -> Result<()> {
    _is_valid_swap(&ctx.accounts.market.coin_wallet, &ctx.accounts.pc_wallet)
}

fn is_valid_swap_transitive(ctx: &Context<SwapTransitive>) -> Result<()> {
    _is_valid_swap(&ctx.accounts.from.coin_wallet, &ctx.accounts.to.coin_wallet)
}

// Validates the tokens being swapped are of different mints.
fn _is_valid_swap<'info>(from: &AccountInfo<'info>, to: &AccountInfo<'info>) -> Result<()> {
    let from_token_mint = token::accessor::mint(from)?;
    let to_token_mint = token::accessor::mint(to)?;
    if from_token_mint == to_token_mint {
        return err!(ErrorCode::SwapTokensCannotMatch);
    }
    Ok(())
}

// Event emitted when a swap occurs for two base currencies on two different
// markets (quoted in the same token).
#[event]
pub struct DidSwap {
    // User given (max) amount to swap.
    pub given_amount: u64,
    // The minimum amount of the *to* token expected to be received from
    // executing the swap.
    pub min_expected_swap_amount: u64,
    // Amount of the `from` token sold.
    pub from_amount: u64,
    // Amount of the `to` token purchased.
    pub to_amount: u64,
    // Amount of the quote currency accumulated from the swap.
    pub spill_amount: u64,
    // Mint sold.
    pub from_mint: Pubkey,
    // Mint purchased.
    pub to_mint: Pubkey,
    // Mint of the token used as the quote currency in the two markets used
    // for swapping.
    pub quote_mint: Pubkey,
    // User that signed the transaction.
    pub authority: Pubkey,
}

#[error_code]
pub enum ErrorCode {
    #[msg("The tokens being swapped must have different mints")]
    SwapTokensCannotMatch,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
}
