use crate::{
    constants::trade::trade::DEFAULT_SLIPPAGE,
    instruction::utils::pumpswap::{
        accounts, fee_recipient_ata, get_user_volume_accumulator_pda, BUY_DISCRIMINATOR,
        SELL_DISCRIMINATOR,
    },
    trading::core::{
        params::{BuyParams, PumpSwapParams, SellParams},
        traits::InstructionBuilder,
    },
    utils::calc::pumpswap::{buy_quote_input_internal, sell_base_input_internal},
};
use anyhow::{anyhow, Result};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signer::Signer,
};

/// Instruction builder for PumpSwap protocol
pub struct PumpSwapInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for PumpSwapInstructionBuilder {
    async fn build_buy_instructions(&self, params: &BuyParams) -> Result<Vec<Instruction>> {
        // Get PumpSwap specific parameters
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpSwap"))?;

        if params.sol_amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        // Build instructions based on account information
        let pool = protocol_params.pool;
        let base_mint = protocol_params.base_mint;
        let quote_mint = protocol_params.quote_mint;
        let pool_base_token_reserves = protocol_params.pool_base_token_reserves;
        let pool_quote_token_reserves = protocol_params.pool_quote_token_reserves;
        let params_coin_creator_vault_ata = protocol_params.coin_creator_vault_ata;
        let params_coin_creator_vault_authority = protocol_params.coin_creator_vault_authority;
        let auto_handle_wsol = protocol_params.auto_handle_wsol;
        let base_token_program = protocol_params.base_token_program;
        let quote_token_program = protocol_params.quote_token_program;
        let pool_base_token_account = protocol_params.pool_base_token_account;
        let pool_quote_token_account = protocol_params.pool_quote_token_account;

        if base_mint != crate::constants::WSOL_TOKEN_ACCOUNT
            && quote_mint != crate::constants::WSOL_TOKEN_ACCOUNT
        {
            return Err(anyhow!("Invalid base mint and quote mint"));
        }

        let quote_mint_is_wsol = quote_mint == crate::constants::WSOL_TOKEN_ACCOUNT;

        let mut token_amount = 0;
        let mut sol_amount = 0;

        let mut creator = Pubkey::default();
        if params_coin_creator_vault_authority != accounts::DEFAULT_COIN_CREATOR_VAULT_AUTHORITY {
            creator = params_coin_creator_vault_authority;
        }
        if quote_mint_is_wsol {
            let result = buy_quote_input_internal(
                params.sol_amount,
                params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
                pool_base_token_reserves,
                pool_quote_token_reserves,
                &creator,
            )
            .unwrap();
            // base_amount_out
            token_amount = result.base;
            // max_quote_amount_in
            sol_amount = result.max_quote;
        } else {
            let result = sell_base_input_internal(
                params.sol_amount,
                params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
                pool_base_token_reserves,
                pool_quote_token_reserves,
                &creator,
            )
            .unwrap();
            // min_quote_amount_out
            token_amount = result.min_quote;
            // base_amount_in
            sol_amount = params.sol_amount;
        }

        // Create user token accounts
        let user_base_token_account =
            crate::common::fast_fn::get_associated_token_address_with_program_id_fast(
                &params.payer.pubkey(),
                &base_mint,
                &base_token_program,
            );
        let user_quote_token_account =
            crate::common::fast_fn::get_associated_token_address_with_program_id_fast(
                &params.payer.pubkey(),
                &quote_mint,
                &quote_token_program,
            );

        let mut instructions = Vec::with_capacity(6);

        if auto_handle_wsol {
            instructions
                .extend(crate::trading::common::handle_wsol(&params.payer.pubkey(), sol_amount));
        }

        // Create user's base token account
        instructions.push(crate::common::fast_fn::create_associated_token_account_idempotent_fast(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            if quote_mint_is_wsol { &base_mint } else { &quote_mint },
            if quote_mint_is_wsol { &base_token_program } else { &quote_token_program },
        ));

        let fee_recipient_ata = fee_recipient_ata(accounts::FEE_RECIPIENT, quote_mint);

        // Create buy instruction
        let mut accounts = Vec::with_capacity(23);
        accounts.extend([
            AccountMeta::new_readonly(pool, false), // pool_id (readonly)
            AccountMeta::new(params.payer.pubkey(), true), // user (signer)
            accounts::GLOBAL_ACCOUNT_META,          // global (readonly)
            AccountMeta::new_readonly(base_mint, false), // base_mint (readonly)
            AccountMeta::new_readonly(quote_mint, false), // quote_mint (readonly)
            AccountMeta::new(user_base_token_account, false), // user_base_token_account
            AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
            AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
            AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
            accounts::FEE_RECIPIENT_META,           // fee_recipient (readonly)
            AccountMeta::new(fee_recipient_ata, false), // fee_recipient_ata
            AccountMeta::new_readonly(base_token_program, false), // TOKEN_PROGRAM_ID (readonly)
            AccountMeta::new_readonly(quote_token_program, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
            crate::constants::SYSTEM_PROGRAM_META,                 // System Program (readonly)
            accounts::ASSOCIATED_TOKEN_PROGRAM_META, // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
            accounts::EVENT_AUTHORITY_META,          // event_authority (readonly)
            accounts::AMM_PROGRAM_META,              // PUMP_AMM_PROGRAM_ID (readonly)
            AccountMeta::new(params_coin_creator_vault_ata, false), // coin_creator_vault_ata
            AccountMeta::new_readonly(params_coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
        ]);
        if quote_mint_is_wsol {
            accounts.push(accounts::GLOBAL_VOLUME_ACCUMULATOR_META);
            accounts.push(AccountMeta::new(
                get_user_volume_accumulator_pda(&params.payer.pubkey()).unwrap(),
                false,
            ));
        }
        accounts.push(accounts::FEE_CONFIG_META);
        accounts.push(accounts::FEE_PROGRAM_META);

        // Create instruction data
        let mut data = [0u8; 24];
        if quote_mint_is_wsol {
            data[..8].copy_from_slice(&BUY_DISCRIMINATOR);
            // base_amount_out
            data[8..16].copy_from_slice(&token_amount.to_le_bytes());
            // max_quote_amount_in
            data[16..24].copy_from_slice(&sol_amount.to_le_bytes());
        } else {
            data[..8].copy_from_slice(&SELL_DISCRIMINATOR);
            // base_amount_in
            data[8..16].copy_from_slice(&sol_amount.to_le_bytes());
            // min_quote_amount_out
            data[16..24].copy_from_slice(&token_amount.to_le_bytes());
        }

        instructions.push(Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data: data.to_vec(),
        });
        if auto_handle_wsol {
            // Close wSOL ATA account, reclaim rent
            instructions.push(crate::trading::common::close_wsol(&params.payer.pubkey()));
        }
        Ok(instructions)
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
        // Get PumpSwap specific parameters
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpSwap"))?;

        // Build instructions based on account information
        let pool = protocol_params.pool;
        let base_mint = protocol_params.base_mint;
        let quote_mint = protocol_params.quote_mint;
        let pool_base_token_reserves = protocol_params.pool_base_token_reserves;
        let pool_quote_token_reserves = protocol_params.pool_quote_token_reserves;
        let pool_base_token_account = protocol_params.pool_base_token_account;
        let pool_quote_token_account = protocol_params.pool_quote_token_account;
        let params_coin_creator_vault_ata = protocol_params.coin_creator_vault_ata;
        let params_coin_creator_vault_authority = protocol_params.coin_creator_vault_authority;
        let auto_handle_wsol = protocol_params.auto_handle_wsol;
        let base_token_program = protocol_params.base_token_program;
        let quote_token_program = protocol_params.quote_token_program;

        if base_mint != crate::constants::WSOL_TOKEN_ACCOUNT
            && quote_mint != crate::constants::WSOL_TOKEN_ACCOUNT
        {
            return Err(anyhow!("Invalid base mint and quote mint"));
        }
        if params.token_amount.is_none() {
            return Err(anyhow!("Token amount is not set"));
        }

        let quote_mint_is_wsol = quote_mint == crate::constants::WSOL_TOKEN_ACCOUNT;

        let mut token_amount = 0;
        let mut sol_amount = 0;

        let mut creator = Pubkey::default();
        if params_coin_creator_vault_authority != accounts::DEFAULT_COIN_CREATOR_VAULT_AUTHORITY {
            creator = params_coin_creator_vault_authority;
        }

        if quote_mint_is_wsol {
            let result = sell_base_input_internal(
                params.token_amount.unwrap(),
                params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
                pool_base_token_reserves,
                pool_quote_token_reserves,
                &creator,
            )
            .unwrap();
            // base_amount_in
            token_amount = params.token_amount.unwrap();
            // min_quote_amount_out
            sol_amount = result.min_quote;
        } else {
            let result = buy_quote_input_internal(
                params.token_amount.unwrap(),
                params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
                pool_base_token_reserves,
                pool_quote_token_reserves,
                &creator,
            )
            .unwrap();
            // max_quote_amount_in
            token_amount = result.max_quote;
            // base_amount_out
            sol_amount = result.base;
        }

        let fee_recipient_ata = fee_recipient_ata(accounts::FEE_RECIPIENT, quote_mint);

        let user_base_token_account =
            crate::common::fast_fn::get_associated_token_address_with_program_id_fast(
                &params.payer.pubkey(),
                &base_mint,
                &base_token_program,
            );
        let user_quote_token_account =
            crate::common::fast_fn::get_associated_token_address_with_program_id_fast(
                &params.payer.pubkey(),
                &quote_mint,
                &quote_token_program,
            );

        let mut instructions = Vec::with_capacity(3);

        // Insert wSOL
        instructions.push(
            // Create wSOL ATA account if it doesn't exist
            crate::common::fast_fn::create_associated_token_account_idempotent_fast(
                &params.payer.pubkey(),
                &params.payer.pubkey(),
                &crate::constants::WSOL_TOKEN_ACCOUNT,
                &crate::constants::TOKEN_PROGRAM,
            ),
        );

        // Create sell instruction
        let mut accounts = Vec::with_capacity(23);
        accounts.extend([
            AccountMeta::new_readonly(pool, false), // pool_id (readonly)
            AccountMeta::new(params.payer.pubkey(), true), // user (signer)
            accounts::GLOBAL_ACCOUNT_META,          // global (readonly)
            AccountMeta::new_readonly(base_mint, false), // mint (readonly)
            AccountMeta::new_readonly(quote_mint, false), // WSOL_TOKEN_ACCOUNT (readonly)
            AccountMeta::new(user_base_token_account, false), // user_base_token_account
            AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
            AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
            AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
            accounts::FEE_RECIPIENT_META,           // fee_recipient (readonly)
            AccountMeta::new(fee_recipient_ata, false), // fee_recipient_ata
            AccountMeta::new_readonly(base_token_program, false), // TOKEN_PROGRAM_ID (readonly)
            AccountMeta::new_readonly(quote_token_program, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
            crate::constants::SYSTEM_PROGRAM_META,                 // System Program (readonly)
            accounts::ASSOCIATED_TOKEN_PROGRAM_META, // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
            accounts::EVENT_AUTHORITY_META,          // event_authority (readonly)
            accounts::AMM_PROGRAM_META,              // PUMP_AMM_PROGRAM_ID (readonly)
            AccountMeta::new(params_coin_creator_vault_ata, false), // coin_creator_vault_ata
            AccountMeta::new_readonly(params_coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
        ]);
        if !quote_mint_is_wsol {
            accounts.push(accounts::GLOBAL_VOLUME_ACCUMULATOR_META);
            accounts.push(AccountMeta::new(
                get_user_volume_accumulator_pda(&params.payer.pubkey()).unwrap(),
                false,
            ));
        }

        accounts.push(accounts::FEE_CONFIG_META);
        accounts.push(accounts::FEE_PROGRAM_META);

        // Create instruction data
        let mut data = [0u8; 24];
        if quote_mint_is_wsol {
            data[..8].copy_from_slice(&SELL_DISCRIMINATOR);
            // base_amount_in
            data[8..16].copy_from_slice(&token_amount.to_le_bytes());
            // min_quote_amount_out
            data[16..24].copy_from_slice(&sol_amount.to_le_bytes());
        } else {
            data[..8].copy_from_slice(&BUY_DISCRIMINATOR);
            // base_amount_out
            data[8..16].copy_from_slice(&sol_amount.to_le_bytes());
            // max_quote_amount_in
            data[16..24].copy_from_slice(&token_amount.to_le_bytes());
        }

        instructions.push(Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data: data.to_vec(),
        });

        if auto_handle_wsol {
            instructions.push(crate::trading::common::close_wsol(&params.payer.pubkey()));
        }
        Ok(instructions)
    }
}
