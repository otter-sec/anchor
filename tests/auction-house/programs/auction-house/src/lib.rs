pub mod utils;

use {
    crate::utils::*,
    anchor_lang::{
        prelude::*,
        solana_program::{
            program::{invoke, invoke_signed},
            system_instruction,
        },
    },
    anchor_spl::{
        associated_token::AssociatedToken,
        token::{
            spl_token::{
                self,
                instruction::{approve, revoke},
            },
            Mint, Token, TokenAccount,
        },
    },
};

declare_program!(mpl_token_metadata);

declare_id!("hausS13jsjafwWwGqZTUQRmWyvyxn9EQpqMwV1PBBmk");

const PREFIX: &str = "auction_house";
const FEE_PAYER: &str = "fee_payer";
const TREASURY: &str = "treasury";
const SIGNER: &str = "signer";
const ZERO: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];

#[program]
pub mod auction_house {
    use super::*;

    pub fn create_auction_house(
        ctx: Context<CreateAuctionHouse>,
        seller_fee_basis_points: u16,
        requires_sign_off: bool,
        can_change_sale_price: bool,
    ) -> Result<()> {
        let treasury_mint = &ctx.accounts.treasury_mint;
        let payer = &ctx.accounts.payer;
        let authority = &ctx.accounts.authority;
        let auction_house = &mut ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let auction_house_treasury = &ctx.accounts.auction_house_treasury;
        let fee_withdrawal_destination = &ctx.accounts.fee_withdrawal_destination;
        let treasury_withdrawal_destination_owner =
            &ctx.accounts.treasury_withdrawal_destination_owner;
        let treasury_withdrawal_destination = &ctx.accounts.treasury_withdrawal_destination;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let associated_token_program = &ctx.accounts.associated_token_program;
        let rent = &ctx.accounts.rent;

        auction_house.treasury_bump = ctx.bumps.auction_house_treasury;
        auction_house.bump = ctx.bumps.auction_house;
        auction_house.fee_payer_bump = ctx.bumps.auction_house_fee_account;

        if seller_fee_basis_points > 10000 {
            return err!(ErrorCode::InvalidBasisPoints);
        }
        auction_house.seller_fee_basis_points = seller_fee_basis_points;
        auction_house.requires_sign_off = requires_sign_off;
        auction_house.can_change_sale_price = can_change_sale_price;
        auction_house.creator = authority.key();
        auction_house.authority = authority.key();
        auction_house.treasury_mint = treasury_mint.key();
        auction_house.auction_house_fee_account = auction_house_fee_account.key();
        auction_house.auction_house_treasury = auction_house_treasury.key();
        auction_house.treasury_withdrawal_destination = treasury_withdrawal_destination.key();
        auction_house.fee_withdrawal_destination = fee_withdrawal_destination.key();

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        let ah_key = auction_house.key();

        let auction_house_treasury_seeds = [
            PREFIX.as_bytes(),
            ah_key.as_ref(),
            TREASURY.as_bytes(),
            &[auction_house.treasury_bump],
        ];

        create_program_token_account_if_not_present(
            auction_house_treasury,
            system_program,
            &payer,
            token_program,
            treasury_mint,
            &auction_house.to_account_info(),
            rent,
            &auction_house_treasury_seeds,
            &[],
            is_native,
        )?;

        if !is_native {
            if treasury_withdrawal_destination.data_is_empty() {
                make_ata(
                    treasury_withdrawal_destination.to_account_info(),
                    treasury_withdrawal_destination_owner.to_account_info(),
                    treasury_mint.to_account_info(),
                    payer.to_account_info(),
                    associated_token_program.to_account_info(),
                    token_program.to_account_info(),
                    system_program.to_account_info(),
                    &[],
                )?;
            }

            assert_is_ata(
                &treasury_withdrawal_destination.to_account_info(),
                &treasury_withdrawal_destination_owner.key(),
                &treasury_mint.key(),
            )?;
        } else {
            assert_keys_equal(
                treasury_withdrawal_destination.key(),
                treasury_withdrawal_destination_owner.key(),
            )?;
        }

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let payment_account = &ctx.accounts.payment_account;
        let transfer_authority = &ctx.accounts.transfer_authority;
        let escrow_payment_account = &ctx.accounts.escrow_payment_account;
        let authority = &ctx.accounts.authority;
        let auction_house = &ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let treasury_mint = &ctx.accounts.treasury_mint;
        let system_program = &ctx.accounts.system_program;
        let token_program = &ctx.accounts.token_program;
        let rent = &ctx.accounts.rent;

        let auction_house_key = auction_house.key();
        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];
        let wallet_key = wallet.key();

        let escrow_payment_bump = ctx.bumps.escrow_payment_account;
        let escrow_signer_seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            wallet_key.as_ref(),
            &[escrow_payment_bump],
        ];

        let (fee_payer, fee_seeds) = get_fee_payer(
            authority,
            auction_house,
            wallet.to_account_info(),
            auction_house_fee_account.to_account_info(),
            &seeds,
        )?;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        create_program_token_account_if_not_present(
            escrow_payment_account,
            system_program,
            &fee_payer,
            token_program,
            treasury_mint,
            &auction_house.to_account_info(),
            rent,
            &escrow_signer_seeds,
            fee_seeds,
            is_native,
        )?;

        if !is_native {
            assert_is_ata(payment_account, &wallet.key(), &treasury_mint.key())?;
            invoke_signed(
                &spl_token::instruction::transfer(
                    token_program.key,
                    &payment_account.key(),
                    &escrow_payment_account.key(),
                    &transfer_authority.key(),
                    &[],
                    amount,
                )?,
                &[
                    escrow_payment_account.to_account_info(),
                    payment_account.to_account_info(),
                    token_program.to_account_info(),
                    transfer_authority.to_account_info(),
                ],
                &[],
            )?;
        } else {
            assert_keys_equal(payment_account.key(), wallet.key())?;
            invoke_signed(
                &system_instruction::transfer(
                    &payment_account.key(),
                    &escrow_payment_account.key(),
                    amount,
                ),
                &[
                    escrow_payment_account.to_account_info(),
                    payment_account.to_account_info(),
                    system_program.to_account_info(),
                ],
                &[],
            )?;
        }

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let receipt_account = &ctx.accounts.receipt_account;
        let escrow_payment_account = &ctx.accounts.escrow_payment_account;
        let authority = &ctx.accounts.authority;
        let auction_house = &ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let treasury_mint = &ctx.accounts.treasury_mint;
        let system_program = &ctx.accounts.system_program;
        let token_program = &ctx.accounts.token_program;
        let associated_token_program = &ctx.accounts.associated_token_program;

        let auction_house_key = auction_house.key();
        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];

        let ah_seeds = [
            PREFIX.as_bytes(),
            auction_house.creator.as_ref(),
            auction_house.treasury_mint.as_ref(),
            &[auction_house.bump],
        ];

        let auction_house_key = auction_house.key();
        let wallet_key = wallet.key();

        if !wallet.to_account_info().is_signer && !authority.to_account_info().is_signer {
            return err!(ErrorCode::NoValidSignerPresent);
        }

        let escrow_payment_bump = ctx.bumps.escrow_payment_account;

        let escrow_signer_seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            wallet_key.as_ref(),
            &[escrow_payment_bump],
        ];

        let (fee_payer, fee_seeds) = get_fee_payer(
            authority,
            auction_house,
            wallet.to_account_info(),
            auction_house_fee_account.to_account_info(),
            &seeds,
        )?;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        if !is_native {
            if receipt_account.data_is_empty() {
                make_ata(
                    receipt_account.to_account_info(),
                    wallet.to_account_info(),
                    treasury_mint.to_account_info(),
                    fee_payer.to_account_info(),
                    associated_token_program.to_account_info(),
                    token_program.to_account_info(),
                    system_program.to_account_info(),
                    &fee_seeds,
                )?;
            }

            let rec_acct = assert_is_ata(
                &receipt_account.to_account_info(),
                &wallet.key(),
                &treasury_mint.key(),
            )?;

            // make sure you can't get rugged
            if rec_acct.delegate.is_some() {
                return err!(ErrorCode::BuyerATACannotHaveDelegate);
            }

            assert_is_ata(receipt_account, &wallet.key(), &treasury_mint.key())?;
            invoke_signed(
                &spl_token::instruction::transfer(
                    token_program.key,
                    &escrow_payment_account.key(),
                    &receipt_account.key(),
                    &auction_house.key(),
                    &[],
                    amount,
                )?,
                &[
                    escrow_payment_account.to_account_info(),
                    receipt_account.to_account_info(),
                    token_program.to_account_info(),
                    auction_house.to_account_info(),
                ],
                &[&ah_seeds],
            )?;
        } else {
            assert_keys_equal(receipt_account.key(), wallet.key())?;
            invoke_signed(
                &system_instruction::transfer(
                    &escrow_payment_account.key(),
                    &receipt_account.key(),
                    amount,
                ),
                &[
                    escrow_payment_account.to_account_info(),
                    receipt_account.to_account_info(),
                    system_program.to_account_info(),
                ],
                &[&escrow_signer_seeds],
            )?;
        }

        Ok(())
    }

    pub fn sell(ctx: Context<Sell>, buyer_price: u64, token_size: u64) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let token_account = &ctx.accounts.token_account;
        let metadata = &ctx.accounts.metadata;
        let authority = &ctx.accounts.authority;
        let seller_trade_state = &ctx.accounts.seller_trade_state;
        let free_seller_trade_state = &ctx.accounts.free_seller_trade_state;
        let auction_house = &ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let program_as_signer = &ctx.accounts.program_as_signer;
        let rent = &ctx.accounts.rent;

        if !wallet.to_account_info().is_signer {
            if buyer_price == 0 {
                return err!(ErrorCode::SaleRequiresSigner);
            } else {
                if free_seller_trade_state.data_is_empty() {
                    return err!(ErrorCode::SaleRequiresSigner);
                } else if !free_seller_trade_state.data_is_empty()
                    && (!authority.to_account_info().is_signer
                        || !auction_house.can_change_sale_price)
                {
                    return err!(ErrorCode::SaleRequiresSigner);
                }
            }
        }

        let auction_house_key = auction_house.key();

        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];

        let (fee_payer, fee_seeds) = get_fee_payer(
            authority,
            auction_house,
            wallet.to_account_info(),
            auction_house_fee_account.to_account_info(),
            &seeds,
        )?;

        assert_is_ata(
            &token_account.to_account_info(),
            &wallet.key(),
            &token_account.mint,
        )?;

        assert_metadata_valid(metadata, token_account)?;

        if token_size > token_account.amount {
            return err!(ErrorCode::InvalidTokenAmount);
        }

        if wallet.is_signer {
            invoke_signed(
                &approve(
                    &token_program.key(),
                    &token_account.key(),
                    &program_as_signer.key(),
                    &wallet.key(),
                    &[],
                    token_size,
                )
                .unwrap(),
                &[
                    token_program.to_account_info(),
                    token_account.to_account_info(),
                    program_as_signer.to_account_info(),
                    wallet.to_account_info(),
                ],
                &[],
            )?;
        }

        let trade_state_bump = ctx.bumps.seller_trade_state;

        let ts_info = seller_trade_state.to_account_info();
        if ts_info.data_is_empty() {
            let token_account_key = token_account.key();
            let wallet_key = wallet.key();
            let ts_seeds = [
                PREFIX.as_bytes(),
                wallet_key.as_ref(),
                auction_house_key.as_ref(),
                token_account_key.as_ref(),
                auction_house.treasury_mint.as_ref(),
                token_account.mint.as_ref(),
                &buyer_price.to_le_bytes(),
                &token_size.to_le_bytes(),
                &[trade_state_bump],
            ];
            create_or_allocate_account_raw(
                *ctx.program_id,
                &ts_info,
                &rent.to_account_info(),
                &system_program,
                &fee_payer,
                TRADE_STATE_SIZE,
                fee_seeds,
                &ts_seeds,
            )?;
        }

        let data = &mut ts_info.data.borrow_mut();
        data[0] = trade_state_bump;

        Ok(())
    }

    pub fn cancel(ctx: Context<Cancel>, _buyer_price: u64, _token_size: u64) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let token_account = &ctx.accounts.token_account;
        let authority = &ctx.accounts.authority;
        let auction_house = &ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let trade_state = &ctx.accounts.trade_state;
        let token_program = &ctx.accounts.token_program;

        if !wallet.to_account_info().is_signer && !authority.to_account_info().is_signer {
            return err!(ErrorCode::NoValidSignerPresent);
        }

        let auction_house_key = auction_house.key();
        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];

        let (fee_payer, _) = get_fee_payer(
            authority,
            auction_house,
            wallet.to_account_info(),
            auction_house_fee_account.to_account_info(),
            &seeds,
        )?;

        let curr_lamp = trade_state.lamports();
        **trade_state.lamports.borrow_mut() = 0;

        **fee_payer.lamports.borrow_mut() = fee_payer
            .lamports()
            .checked_add(curr_lamp)
            .ok_or(error!(ErrorCode::NumericalOverflow))?;

        if token_account.owner == wallet.key() && wallet.is_signer {
            invoke_signed(
                &revoke(
                    &token_program.key(),
                    &token_account.key(),
                    &wallet.key(),
                    &[],
                )
                .unwrap(),
                &[
                    token_program.to_account_info(),
                    token_account.to_account_info(),
                    wallet.to_account_info(),
                ],
                &[],
            )?;
        }

        Ok(())
    }

    pub fn buy(ctx: Context<Buy>, buyer_price: u64, token_size: u64) -> Result<()> {
        let wallet = &ctx.accounts.wallet;
        let payment_account = &ctx.accounts.payment_account;
        let transfer_authority = &ctx.accounts.transfer_authority;
        let treasury_mint = &ctx.accounts.treasury_mint;
        let metadata = &ctx.accounts.metadata;
        let token_account = &ctx.accounts.token_account;
        let escrow_payment_account = &ctx.accounts.escrow_payment_account;
        let authority = &ctx.accounts.authority;
        let auction_house = &ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let buyer_trade_state = &mut ctx.accounts.buyer_trade_state;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let rent = &ctx.accounts.rent;

        let trade_state_bump = ctx.bumps.buyer_trade_state;
        let escrow_payment_bump = ctx.bumps.escrow_payment_account;

        let auction_house_key = auction_house.key();
        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];

        let (fee_payer, fee_seeds) = get_fee_payer(
            authority,
            auction_house,
            wallet.to_account_info(),
            auction_house_fee_account.to_account_info(),
            &seeds,
        )?;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        let auction_house_key = auction_house.key();
        let wallet_key = wallet.key();
        let escrow_signer_seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            wallet_key.as_ref(),
            &[escrow_payment_bump],
        ];

        create_program_token_account_if_not_present(
            escrow_payment_account,
            system_program,
            &fee_payer,
            token_program,
            treasury_mint,
            &auction_house.to_account_info(),
            rent,
            &escrow_signer_seeds,
            fee_seeds,
            is_native,
        )?;

        if is_native {
            assert_keys_equal(wallet.key(), payment_account.key())?;

            if escrow_payment_account.lamports() < buyer_price {
                let diff = buyer_price
                    .checked_sub(escrow_payment_account.lamports())
                    .ok_or(error!(ErrorCode::NumericalOverflow))?;
                invoke_signed(
                    &system_instruction::transfer(
                        &payment_account.key(),
                        &escrow_payment_account.key(),
                        diff,
                    ),
                    &[
                        payment_account.to_account_info(),
                        escrow_payment_account.to_account_info(),
                        system_program.to_account_info(),
                    ],
                    &[],
                )?;
            }
        } else {
            let escrow_payment_loaded: spl_token::state::Account =
                assert_initialized(escrow_payment_account)?;

            if escrow_payment_loaded.amount < buyer_price {
                let diff = buyer_price
                    .checked_sub(escrow_payment_loaded.amount)
                    .ok_or(error!(ErrorCode::NumericalOverflow))?;
                invoke(
                    &spl_token::instruction::transfer(
                        &token_program.key(),
                        &payment_account.key(),
                        &escrow_payment_account.key(),
                        &transfer_authority.key(),
                        &[],
                        diff,
                    )?,
                    &[
                        transfer_authority.to_account_info(),
                        payment_account.to_account_info(),
                        escrow_payment_account.to_account_info(),
                        token_program.to_account_info(),
                    ],
                )?;
            }
        }

        assert_metadata_valid(metadata, token_account)?;

        let ts_info = buyer_trade_state.to_account_info();
        if ts_info.data_is_empty() {
            let token_account_key = token_account.key();
            let wallet_key = wallet.key();
            let ts_seeds = [
                PREFIX.as_bytes(),
                wallet_key.as_ref(),
                auction_house_key.as_ref(),
                token_account_key.as_ref(),
                auction_house.treasury_mint.as_ref(),
                token_account.mint.as_ref(),
                &buyer_price.to_le_bytes(),
                &token_size.to_le_bytes(),
                &[trade_state_bump],
            ];
            create_or_allocate_account_raw(
                *ctx.program_id,
                &ts_info,
                &rent.to_account_info(),
                &system_program,
                &fee_payer,
                TRADE_STATE_SIZE,
                fee_seeds,
                &ts_seeds,
            )?;
        }
        let data = &mut ts_info.data.borrow_mut();
        data[0] = trade_state_bump;

        Ok(())
    }

    pub fn execute_sale<'info>(
        ctx: Context<'info, ExecuteSale<'info>>,
        buyer_price: u64,
        token_size: u64,
    ) -> Result<()> {
        let buyer = &ctx.accounts.buyer;
        let seller = &ctx.accounts.seller;
        let token_account = &ctx.accounts.token_account;
        let token_mint = &ctx.accounts.token_mint;
        let metadata = &ctx.accounts.metadata;
        let treasury_mint = &ctx.accounts.treasury_mint;
        let seller_payment_receipt_account = &ctx.accounts.seller_payment_receipt_account;
        let buyer_receipt_token_account = &ctx.accounts.buyer_receipt_token_account;
        let escrow_payment_account = &ctx.accounts.escrow_payment_account;
        let authority = &ctx.accounts.authority;
        let auction_house = &ctx.accounts.auction_house;
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let auction_house_treasury = &ctx.accounts.auction_house_treasury;
        let buyer_trade_state = &ctx.accounts.buyer_trade_state;
        let seller_trade_state = &ctx.accounts.seller_trade_state;
        let free_trade_state = &ctx.accounts.free_trade_state;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let associated_token_program = &ctx.accounts.associated_token_program;
        let program_as_signer = &ctx.accounts.program_as_signer;

        let auction_house_clone = auction_house.to_account_info();
        let ata_clone = associated_token_program.to_account_info();
        let token_clone = token_program.to_account_info();

        let escrow_payment_bump = ctx.bumps.escrow_payment_account;
        let program_as_signer_bump = ctx.bumps.program_as_signer;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        if buyer_price == 0 && !authority.is_signer && !seller.is_signer {
            return err!(ErrorCode::CannotMatchFreeSalesWithoutAuctionHouseOrSellerSignoff);
        }

        let token_account_mint = get_mint_from_token_account(&token_account)?;

        assert_keys_equal(token_mint.key(), token_account_mint)?;
        let delegate = get_delegate_from_token_account(&token_account)?;
        if let Some(d) = delegate {
            assert_keys_equal(program_as_signer.key(), d)?;
        } else {
            msg!("No delegate detected on token account.");
            return err!(ErrorCode::BothPartiesNeedToAgreeToSale);
        }

        if buyer_trade_state.data_is_empty() || seller_trade_state.data_is_empty() {
            return err!(ErrorCode::BothPartiesNeedToAgreeToSale);
        }

        let auction_house_key = auction_house.key();
        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];

        let wallet_to_use = if buyer.is_signer { buyer } else { seller };

        let (fee_payer, fee_payer_seeds) = get_fee_payer(
            authority,
            auction_house,
            wallet_to_use.to_account_info(),
            auction_house_fee_account.to_account_info(),
            &seeds,
        )?;
        let fee_payer_clone = fee_payer.to_account_info();

        assert_is_ata(
            &token_account.to_account_info(),
            &seller.key(),
            &token_account_mint,
        )?;

        assert_derivation(
            &mpl_token_metadata::ID,
            &metadata.to_account_info(),
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                token_account_mint.as_ref(),
            ],
        )?;

        if metadata.data_is_empty() {
            return err!(ErrorCode::MetadataDoesntExist);
        }

        let wallet_key = buyer.key();
        let escrow_signer_seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            wallet_key.as_ref(),
            &[escrow_payment_bump],
        ];

        let ah_seeds = [
            PREFIX.as_bytes(),
            auction_house.creator.as_ref(),
            auction_house.treasury_mint.as_ref(),
            &[auction_house.bump],
        ];

        // with the native account, the escrow is its own owner,
        // whereas with token, it is the auction house that is owner.
        let signer_seeds_for_royalties = if is_native {
            escrow_signer_seeds
        } else {
            ah_seeds
        };

        let buyer_leftover_after_royalties = pay_creator_fees(
            &mut ctx.remaining_accounts.iter(),
            &metadata,
            &escrow_payment_account,
            &auction_house_clone,
            &fee_payer_clone,
            treasury_mint,
            &ata_clone,
            &token_clone,
            system_program.as_ref(),
            &signer_seeds_for_royalties,
            &fee_payer_seeds,
            buyer_price,
            is_native,
        )?;

        let auction_house_fee_paid = pay_auction_house_fees(
            &auction_house,
            &auction_house_treasury,
            &escrow_payment_account,
            &token_clone,
            system_program.as_ref(),
            &signer_seeds_for_royalties,
            buyer_price,
            is_native,
        )?;

        let buyer_leftover_after_royalties_and_house_fee = buyer_leftover_after_royalties
            .checked_sub(auction_house_fee_paid)
            .ok_or(error!(ErrorCode::NumericalOverflow))?;

        if !is_native {
            if seller_payment_receipt_account.data_is_empty() {
                make_ata(
                    seller_payment_receipt_account.to_account_info(),
                    seller.to_account_info(),
                    treasury_mint.to_account_info(),
                    fee_payer.to_account_info(),
                    associated_token_program.to_account_info(),
                    token_program.to_account_info(),
                    system_program.to_account_info(),
                    &fee_payer_seeds,
                )?;
            }

            let seller_rec_acct = assert_is_ata(
                &seller_payment_receipt_account.to_account_info(),
                &seller.key(),
                &treasury_mint.key(),
            )?;

            // make sure you can't get rugged
            if seller_rec_acct.delegate.is_some() {
                return err!(ErrorCode::SellerATACannotHaveDelegate);
            }

            invoke_signed(
                &spl_token::instruction::transfer(
                    token_program.key,
                    &escrow_payment_account.key(),
                    &seller_payment_receipt_account.key(),
                    &auction_house.key(),
                    &[],
                    buyer_leftover_after_royalties_and_house_fee,
                )?,
                &[
                    escrow_payment_account.to_account_info(),
                    seller_payment_receipt_account.to_account_info(),
                    token_program.to_account_info(),
                    auction_house.to_account_info(),
                ],
                &[&ah_seeds],
            )?;
        } else {
            assert_keys_equal(seller_payment_receipt_account.key(), seller.key())?;
            invoke_signed(
                &system_instruction::transfer(
                    &escrow_payment_account.key,
                    seller_payment_receipt_account.key,
                    buyer_leftover_after_royalties_and_house_fee,
                ),
                &[
                    escrow_payment_account.to_account_info(),
                    seller_payment_receipt_account.to_account_info(),
                    system_program.to_account_info(),
                ],
                &[&escrow_signer_seeds],
            )?;
        }

        if buyer_receipt_token_account.data_is_empty() {
            make_ata(
                buyer_receipt_token_account.to_account_info(),
                buyer.to_account_info(),
                token_mint.to_account_info(),
                fee_payer.to_account_info(),
                associated_token_program.to_account_info(),
                token_program.to_account_info(),
                system_program.to_account_info(),
                &fee_payer_seeds,
            )?;
        }

        let buyer_rec_acct = assert_is_ata(
            &buyer_receipt_token_account,
            &buyer.key(),
            &token_mint.key(),
        )?;

        // make sure you can't get rugged
        if buyer_rec_acct.delegate.is_some() {
            return err!(ErrorCode::BuyerATACannotHaveDelegate);
        }

        let program_as_signer_seeds = [
            PREFIX.as_bytes(),
            SIGNER.as_bytes(),
            &[program_as_signer_bump],
        ];

        invoke_signed(
            &spl_token::instruction::transfer(
                token_program.key,
                &token_account.key(),
                &buyer_receipt_token_account.key(),
                &program_as_signer.key(),
                &[],
                token_size,
            )?,
            &[
                token_account.to_account_info(),
                buyer_receipt_token_account.to_account_info(),
                program_as_signer.to_account_info(),
                token_clone,
            ],
            &[&program_as_signer_seeds],
        )?;

        let curr_seller_lamp = seller_trade_state.lamports();
        **seller_trade_state.lamports.borrow_mut() = 0;

        **fee_payer.lamports.borrow_mut() = fee_payer
            .lamports()
            .checked_add(curr_seller_lamp)
            .ok_or(error!(ErrorCode::NumericalOverflow))?;

        let curr_buyer_lamp = buyer_trade_state.lamports();
        **buyer_trade_state.lamports.borrow_mut() = 0;

        **fee_payer.lamports.borrow_mut() = fee_payer
            .lamports()
            .checked_add(curr_buyer_lamp)
            .ok_or(error!(ErrorCode::NumericalOverflow))?;

        if free_trade_state.lamports() > 0 {
            let curr_buyer_lamp = free_trade_state.lamports();
            **free_trade_state.lamports.borrow_mut() = 0;

            **fee_payer.lamports.borrow_mut() = fee_payer
                .lamports()
                .checked_add(curr_buyer_lamp)
                .ok_or(error!(ErrorCode::NumericalOverflow))?;
        }
        Ok(())
    }

    pub fn withdraw_from_fee(ctx: Context<WithdrawFromFee>, amount: u64) -> Result<()> {
        let auction_house_fee_account = &ctx.accounts.auction_house_fee_account;
        let fee_withdrawal_destination = &ctx.accounts.fee_withdrawal_destination;
        let auction_house = &ctx.accounts.auction_house;
        let system_program = &ctx.accounts.system_program;

        let auction_house_key = auction_house.key();
        let seeds = [
            PREFIX.as_bytes(),
            auction_house_key.as_ref(),
            FEE_PAYER.as_bytes(),
            &[auction_house.fee_payer_bump],
        ];

        invoke_signed(
            &system_instruction::transfer(
                &auction_house_fee_account.key(),
                &fee_withdrawal_destination.key(),
                amount,
            ),
            &[
                auction_house_fee_account.to_account_info(),
                fee_withdrawal_destination.to_account_info(),
                system_program.to_account_info(),
            ],
            &[&seeds],
        )?;

        Ok(())
    }

    pub fn withdraw_from_treasury(ctx: Context<WithdrawFromTreasury>, amount: u64) -> Result<()> {
        let treasury_mint = &ctx.accounts.treasury_mint;
        let treasury_withdrawal_destination = &ctx.accounts.treasury_withdrawal_destination;
        let auction_house_treasury = &ctx.accounts.auction_house_treasury;
        let auction_house = &ctx.accounts.auction_house;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();
        let auction_house_seeds = [
            PREFIX.as_bytes(),
            auction_house.creator.as_ref(),
            auction_house.treasury_mint.as_ref(),
            &[auction_house.bump],
        ];

        let ah_key = auction_house.key();
        let auction_house_treasury_seeds = [
            PREFIX.as_bytes(),
            ah_key.as_ref(),
            TREASURY.as_bytes(),
            &[auction_house.treasury_bump],
        ];
        if !is_native {
            invoke_signed(
                &spl_token::instruction::transfer(
                    token_program.key,
                    &auction_house_treasury.key(),
                    &treasury_withdrawal_destination.key(),
                    &auction_house.key(),
                    &[],
                    amount,
                )?,
                &[
                    auction_house_treasury.to_account_info(),
                    treasury_withdrawal_destination.to_account_info(),
                    token_program.to_account_info(),
                    auction_house.to_account_info(),
                ],
                &[&auction_house_seeds],
            )?;
        } else {
            invoke_signed(
                &system_instruction::transfer(
                    &auction_house_treasury.key(),
                    &treasury_withdrawal_destination.key(),
                    amount,
                ),
                &[
                    auction_house_treasury.to_account_info(),
                    treasury_withdrawal_destination.to_account_info(),
                    system_program.to_account_info(),
                ],
                &[&auction_house_treasury_seeds],
            )?;
        }

        Ok(())
    }

    pub fn update_auction_house(
        ctx: Context<UpdateAuctionHouse>,
        seller_fee_basis_points: Option<u16>,
        requires_sign_off: Option<bool>,
        can_change_sale_price: Option<bool>,
    ) -> Result<()> {
        let treasury_mint = &ctx.accounts.treasury_mint;
        let payer = &ctx.accounts.payer;
        let new_authority = &ctx.accounts.new_authority;
        let auction_house = &mut ctx.accounts.auction_house;
        let fee_withdrawal_destination = &ctx.accounts.fee_withdrawal_destination;
        let treasury_withdrawal_destination_owner =
            &ctx.accounts.treasury_withdrawal_destination_owner;
        let treasury_withdrawal_destination = &ctx.accounts.treasury_withdrawal_destination;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let associated_token_program = &ctx.accounts.associated_token_program;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        if let Some(sfbp) = seller_fee_basis_points {
            if sfbp > 10000 {
                return err!(ErrorCode::InvalidBasisPoints);
            }

            auction_house.seller_fee_basis_points = sfbp;
        }

        if let Some(rqf) = requires_sign_off {
            auction_house.requires_sign_off = rqf;
        }
        if let Some(chsp) = can_change_sale_price {
            auction_house.can_change_sale_price = chsp;
        }

        auction_house.authority = new_authority.key();
        auction_house.treasury_withdrawal_destination = treasury_withdrawal_destination.key();
        auction_house.fee_withdrawal_destination = fee_withdrawal_destination.key();

        if !is_native {
            if treasury_withdrawal_destination.data_is_empty() {
                make_ata(
                    treasury_withdrawal_destination.to_account_info(),
                    treasury_withdrawal_destination_owner.to_account_info(),
                    treasury_mint.to_account_info(),
                    payer.to_account_info(),
                    associated_token_program.to_account_info(),
                    token_program.to_account_info(),
                    system_program.to_account_info(),
                    &[],
                )?;
            }

            assert_is_ata(
                &treasury_withdrawal_destination.to_account_info(),
                &treasury_withdrawal_destination_owner.key(),
                &treasury_mint.key(),
            )?;
        } else {
            assert_keys_equal(
                treasury_withdrawal_destination.key(),
                treasury_withdrawal_destination_owner.key(),
            )?;
        }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateAuctionHouse<'info> {
    treasury_mint: Account<'info, Mint>,
    #[account(mut)]
    payer: Signer<'info>,
    authority: AccountInfo<'info>,
    #[account(mut)]
    fee_withdrawal_destination: UncheckedAccount<'info>,
    treasury_withdrawal_destination_owner: UncheckedAccount<'info>,
    #[account(mut)]
    treasury_withdrawal_destination: UncheckedAccount<'info>,
    #[account(
        init,
        seeds=[
            PREFIX.as_bytes(),
            authority.key().as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump,
        space=AUCTION_HOUSE_SIZE,
        payer=payer,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            TREASURY.as_bytes(),
        ],
        bump,
    )]
    auction_house_treasury: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    associated_token_program: Program<'info, AssociatedToken>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    wallet: Signer<'info>,
    #[account(mut)]
    payment_account: UncheckedAccount<'info>,
    transfer_authority: UncheckedAccount<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump,
        has_one=authority,
        has_one=treasury_mint,
        has_one=auction_house_fee_account,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            wallet.key().as_ref(),
        ],
        bump,
    )]
    escrow_payment_account: UncheckedAccount<'info>,
    treasury_mint: Account<'info, Mint>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    wallet: UncheckedAccount<'info>,
    #[account(mut)]
    receipt_account: UncheckedAccount<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            authority.key().as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=treasury_mint,
        has_one=auction_house_fee_account,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            wallet.key().as_ref(),
        ],
        bump,
    )]
    escrow_payment_account: UncheckedAccount<'info>,
    treasury_mint: Account<'info, Mint>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(buyer_price: u64, token_size: u64)]
pub struct Sell<'info> {
    wallet: UncheckedAccount<'info>,
    #[account(mut)]
    token_account: Account<'info, TokenAccount>,
    metadata: UncheckedAccount<'info>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            authority.key().as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=auction_house_fee_account,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            wallet.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            auction_house.treasury_mint.as_ref(),
            token_account.mint.as_ref(),
            buyer_price.to_le_bytes().as_ref(),
            token_size.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    seller_trade_state: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            wallet.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            auction_house.treasury_mint.as_ref(),
            token_account.mint.as_ref(),
            ZERO.as_ref(),
            token_size.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    free_seller_trade_state: UncheckedAccount<'info>,
    treasury_mint: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            SIGNER.as_bytes(),
        ],
        bump,
    )]
    program_as_signer: UncheckedAccount<'info>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(buyer_price: u64, token_size: u64)]
pub struct Cancel<'info> {
    #[account(mut)]
    wallet: UncheckedAccount<'info>,
    #[account(mut)]
    token_account: Account<'info, TokenAccount>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    treasury_mint: UncheckedAccount<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key.as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=auction_house_fee_account,
        has_one=treasury_mint,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            wallet.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            auction_house.treasury_mint.as_ref(),
            token_account.mint.as_ref(),
            buyer_price.to_le_bytes().as_ref(),
            token_size.to_le_bytes().as_ref(),
        ],
        bump=trade_state.to_account_info().data.borrow()[0],
    )]
    trade_state: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(buyer_price: u64, token_size: u64)]
pub struct Buy<'info> {
    wallet: Signer<'info>,
    #[account(mut)]
    payment_account: UncheckedAccount<'info>,
    transfer_authority: UncheckedAccount<'info>,
    treasury_mint: Account<'info, Mint>,
    token_account: Account<'info, TokenAccount>,
    metadata: UncheckedAccount<'info>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=treasury_mint,
        has_one=auction_house_fee_account,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            wallet.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            treasury_mint.key().as_ref(),
            token_account.mint.as_ref(),
            &buyer_price.to_le_bytes(),
            &token_size.to_le_bytes(),
        ],
        bump,
    )]
    buyer_trade_state: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            wallet.key().as_ref(),
        ],
        bump,
    )]
    escrow_payment_account: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(buyer_price: u64, token_size: u64)]
pub struct ExecuteSale<'info> {
    #[account(mut)]
    buyer: UncheckedAccount<'info>,
    #[account(mut)]
    seller: UncheckedAccount<'info>,
    // cannot mark these as real Accounts or else we blow stack size limit
    #[account(mut)]
    token_account: UncheckedAccount<'info>,
    token_mint: UncheckedAccount<'info>,
    metadata: UncheckedAccount<'info>,
    // cannot mark these as real Accounts or else we blow stack size limit
    treasury_mint: UncheckedAccount<'info>,
    #[account(mut)]
    seller_payment_receipt_account: UncheckedAccount<'info>,
    #[account(mut)]
    buyer_receipt_token_account: UncheckedAccount<'info>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key.as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=treasury_mint,
        has_one=auction_house_treasury,
        has_one=auction_house_fee_account,
    )]
    auction_house: Box<Account<'info, AuctionHouse>>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            TREASURY.as_bytes(),
        ],
        bump=auction_house.treasury_bump,
    )]
    auction_house_treasury: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            buyer.key().as_ref(),
        ],
        bump,
    )]
    escrow_payment_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            buyer.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            auction_house.treasury_mint.as_ref(),
            token_mint.key().as_ref(),
            buyer_price.to_le_bytes().as_ref(),
            token_size.to_le_bytes().as_ref(),
        ],
        bump=buyer_trade_state.to_account_info().data.borrow()[0],
    )]
    buyer_trade_state: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            seller.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            auction_house.treasury_mint.as_ref(),
            token_mint.key().as_ref(),
            buyer_price.to_le_bytes().as_ref(),
            token_size.to_le_bytes().as_ref(),
        ],
        bump=seller_trade_state.to_account_info().data.borrow()[0],
    )]
    seller_trade_state: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            seller.key().as_ref(),
            auction_house.key().as_ref(),
            token_account.key().as_ref(),
            auction_house.treasury_mint.as_ref(),
            token_mint.key().as_ref(),
            ZERO.as_ref(),
            token_size.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    free_trade_state: UncheckedAccount<'info>,
    #[account(
        seeds=[
            PREFIX.as_bytes(),
            SIGNER.as_bytes(),
        ],
        bump,
    )]
    program_as_signer: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct WithdrawFromTreasury<'info> {
    treasury_mint: Account<'info, Mint>,
    authority: Signer<'info>,
    #[account(mut)]
    treasury_withdrawal_destination: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=treasury_mint,
        has_one=treasury_withdrawal_destination,
        has_one=auction_house_treasury,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            TREASURY.as_bytes(),
        ],
        bump=auction_house.treasury_bump,
    )]
    auction_house_treasury: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawFromFee<'info> {
    authority: Signer<'info>,
    treasury_mint: UncheckedAccount<'info>,
    #[account(mut)]
    fee_withdrawal_destination: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=fee_withdrawal_destination,
        has_one=auction_house_fee_account,
    )]
    auction_house: Account<'info, AuctionHouse>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            auction_house.key().as_ref(),
            FEE_PAYER.as_bytes(),
        ],
        bump=auction_house.fee_payer_bump,
    )]
    auction_house_fee_account: UncheckedAccount<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAuctionHouse<'info> {
    treasury_mint: Account<'info, Mint>,
    payer: Signer<'info>,
    authority: Signer<'info>,
    new_authority: UncheckedAccount<'info>,
    #[account(mut)]
    fee_withdrawal_destination: UncheckedAccount<'info>,
    #[account(mut)]
    treasury_withdrawal_destination: UncheckedAccount<'info>,
    treasury_withdrawal_destination_owner: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds=[
            PREFIX.as_bytes(),
            authority.key.as_ref(),
            treasury_mint.key().as_ref(),
        ],
        bump=auction_house.bump,
        has_one=authority,
        has_one=treasury_mint,
    )]
    auction_house: Account<'info, AuctionHouse>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    associated_token_program: Program<'info, AssociatedToken>,
}

pub const AUCTION_HOUSE_SIZE: usize = 8 + //key
32 + //fee payer
32 + //treasury
32 + //treasury_withdrawal_destination
32 + //fee withdrawal destination
32 + //treasury mint
32 + //authority
32 + // creator
1 + // bump
1 + // treasury_bump
1 + // fee_payer_bump
2 + // seller fee basis points
1 + // requires sign off
1 + // can change sale price
220; //padding

#[account]
pub struct AuctionHouse {
    pub auction_house_fee_account: Pubkey,
    pub auction_house_treasury: Pubkey,
    pub treasury_withdrawal_destination: Pubkey,
    pub fee_withdrawal_destination: Pubkey,
    pub treasury_mint: Pubkey,
    pub authority: Pubkey,
    pub creator: Pubkey,
    pub bump: u8,
    pub treasury_bump: u8,
    pub fee_payer_bump: u8,
    pub seller_fee_basis_points: u16,
    pub requires_sign_off: bool,
    pub can_change_sale_price: bool,
}

pub const TRADE_STATE_SIZE: usize = 1;

#[error_code]
pub enum ErrorCode {
    #[msg("PublicKeyMismatch")]
    PublicKeyMismatch,
    #[msg("InvalidMintAuthority")]
    InvalidMintAuthority,
    #[msg("UninitializedAccount")]
    UninitializedAccount,
    #[msg("IncorrectOwner")]
    IncorrectOwner,
    #[msg("PublicKeysShouldBeUnique")]
    PublicKeysShouldBeUnique,
    #[msg("StatementFalse")]
    StatementFalse,
    #[msg("NotRentExempt")]
    NotRentExempt,
    #[msg("NumericalOverflow")]
    NumericalOverflow,
    #[msg("Expected a sol account but got an spl token account instead")]
    ExpectedSolAccount,
    #[msg("Cannot exchange sol for sol")]
    CannotExchangeSOLForSol,
    #[msg("If paying with sol, sol wallet must be signer")]
    SOLWalletMustSign,
    #[msg("Cannot take this action without auction house signing too")]
    CannotTakeThisActionWithoutAuctionHouseSignOff,
    #[msg("No payer present on this txn")]
    NoPayerPresent,
    #[msg("Derived key invalid")]
    DerivedKeyInvalid,
    #[msg("Metadata doesn't exist")]
    MetadataDoesntExist,
    #[msg("Invalid token amount")]
    InvalidTokenAmount,
    #[msg("Both parties need to agree to this sale")]
    BothPartiesNeedToAgreeToSale,
    #[msg("Cannot match free sales unless the auction house or seller signs off")]
    CannotMatchFreeSalesWithoutAuctionHouseOrSellerSignoff,
    #[msg("This sale requires a signer")]
    SaleRequiresSigner,
    #[msg("Old seller not initialized")]
    OldSellerNotInitialized,
    #[msg("Seller ata cannot have a delegate set")]
    SellerATACannotHaveDelegate,
    #[msg("Buyer ata cannot have a delegate set")]
    BuyerATACannotHaveDelegate,
    #[msg("No valid signer present")]
    NoValidSignerPresent,
    #[msg("BP must be less than or equal to 10000")]
    InvalidBasisPoints,
}
