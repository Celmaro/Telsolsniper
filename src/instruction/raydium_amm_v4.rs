use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, signer::Signer};
use solana_system_interface::instruction::transfer;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::instruction::close_account;

use crate::{
    constants::{
        raydium_amm_v4::{accounts, SWAP_BASE_IN_DISCRIMINATOR},
        trade::trade::DEFAULT_SLIPPAGE,
    },
    trading::core::{
        params::{BuyParams, RaydiumAmmV4Params, SellParams},
        traits::InstructionBuilder,
    },
    utils::calc::raydium_amm_v4::compute_swap_amount,
};

/// Instruction builder for RaydiumCpmm protocol
pub struct RaydiumAmmV4InstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for RaydiumAmmV4InstructionBuilder {
    async fn build_buy_instructions(&self, params: &BuyParams) -> Result<Vec<Instruction>> {
        if params.sol_amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }
        self.build_buy_instructions_with_accounts(params).await
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
        self.build_sell_instructions_with_accounts(params).await
    }
}

impl RaydiumAmmV4InstructionBuilder {
    /// Build buy instructions with provided account information
    async fn build_buy_instructions_with_accounts(
        &self,
        params: &BuyParams,
    ) -> Result<Vec<Instruction>> {
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<RaydiumAmmV4Params>()
            .ok_or_else(|| anyhow!("Invalid protocol params for RaydiumCpmm"))?;

        let wsol_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );

        let is_base_in = protocol_params.coin_mint == accounts::WSOL_TOKEN_ACCOUNT;

        let amount_in: u64 = params.sol_amount;
        let swap_result = compute_swap_amount(
            protocol_params.coin_reserve,
            protocol_params.pc_reserve,
            is_base_in,
            amount_in,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let minimum_amount_out = swap_result.min_amount_out;

        let mut instructions = vec![];

        if protocol_params.auto_handle_wsol {
            // Handle wSOL
            instructions.push(
                // Create wSOL ATA account if it doesn't exist
                create_associated_token_account_idempotent(
                    &params.payer.pubkey(),
                    &params.payer.pubkey(),
                    &accounts::WSOL_TOKEN_ACCOUNT,
                    &accounts::TOKEN_PROGRAM,
                ),
            );
            instructions.push(
                // Transfer SOL to wSOL ATA account
                transfer(&params.payer.pubkey(), &wsol_token_account, amount_in),
            );

            // Sync wSOL balance
            instructions.push(
                spl_token::instruction::sync_native(&accounts::TOKEN_PROGRAM, &wsol_token_account)
                    .unwrap(),
            );
        }

        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &accounts::TOKEN_PROGRAM,
        ));

        let user_source_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );
        let user_destination_token_account =
            spl_associated_token_account::get_associated_token_address(
                &params.payer.pubkey(),
                &params.mint,
            );

        // Create buy instruction
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Amm
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AUTHORITY, false), // Authority (readonly)
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Amm Open Orders
            solana_sdk::instruction::AccountMeta::new(protocol_params.token_coin, false), // Pool Coin Token Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.token_pc, false), // Pool Pc Token Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Program
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Market
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Bids
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Asks
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Event Queue
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Coin Vault Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Pc Vault Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Vault Signer
            solana_sdk::instruction::AccountMeta::new(user_source_token_account, false), // User Source Token Account
            solana_sdk::instruction::AccountMeta::new(user_destination_token_account, false), // User Destination Token Account
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // User Source Owner
        ];
        // Create instruction data
        let mut data = vec![];
        data.extend_from_slice(&SWAP_BASE_IN_DISCRIMINATOR);
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());

        instructions.push(Instruction { program_id: accounts::RAYDIUM_AMM_V4, accounts, data });

        if protocol_params.auto_handle_wsol {
            // Close wSOL ATA account, reclaim rent
            instructions.push(
                spl_token::instruction::close_account(
                    &accounts::TOKEN_PROGRAM,
                    &wsol_token_account,
                    &params.payer.pubkey(),
                    &params.payer.pubkey(),
                    &[],
                )
                .unwrap(),
            );
        }

        Ok(instructions)
    }

    /// Build sell instructions with provided account information
    async fn build_sell_instructions_with_accounts(
        &self,
        params: &SellParams,
    ) -> Result<Vec<Instruction>> {
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<RaydiumAmmV4Params>()
            .ok_or_else(|| anyhow!("Invalid protocol params for RaydiumCpmm"))?;

        if params.token_amount.is_none() || params.token_amount.unwrap_or(0) == 0 {
            return Err(anyhow!("Token amount is not set"));
        }

        let wsol_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );

        let is_base_in = protocol_params.pc_mint == accounts::WSOL_TOKEN_ACCOUNT;
        let swap_result = compute_swap_amount(
            protocol_params.coin_reserve,
            protocol_params.pc_reserve,
            is_base_in,
            params.token_amount.unwrap_or(0),
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let minimum_amount_out = swap_result.min_amount_out;

        let mut instructions = vec![];

        // Handle wSOL
        instructions.push(
            // Create wSOL ATA account if it doesn't exist
            create_associated_token_account_idempotent(
                &params.payer.pubkey(),
                &params.payer.pubkey(),
                &accounts::WSOL_TOKEN_ACCOUNT,
                &accounts::TOKEN_PROGRAM,
            ),
        );

        let user_source_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );
        let user_destination_token_account =
            spl_associated_token_account::get_associated_token_address(
                &params.payer.pubkey(),
                &accounts::WSOL_TOKEN_ACCOUNT,
            );

        // Create buy instruction
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Amm
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AUTHORITY, false), // Authority (readonly)
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Amm Open Orders
            solana_sdk::instruction::AccountMeta::new(protocol_params.token_coin, false), // Pool Coin Token Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.token_pc, false), // Pool Pc Token Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Program
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Market
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Bids
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Asks
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Event Queue
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Coin Vault Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Pc Vault Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.amm, false), // Serum Vault Signer
            solana_sdk::instruction::AccountMeta::new(user_source_token_account, false), // User Source Token Account
            solana_sdk::instruction::AccountMeta::new(user_destination_token_account, false), // User Destination Token Account
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // User Source Owner
        ];
        // Create instruction data
        let mut data = vec![];
        data.extend_from_slice(&SWAP_BASE_IN_DISCRIMINATOR);
        data.extend_from_slice(&params.token_amount.unwrap_or(0).to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());

        instructions.push(Instruction { program_id: accounts::RAYDIUM_AMM_V4, accounts, data });

        if protocol_params.auto_handle_wsol {
            instructions.push(
                close_account(
                    &accounts::TOKEN_PROGRAM,
                    &wsol_token_account,
                    &params.payer.pubkey(),
                    &params.payer.pubkey(),
                    &[&params.payer.pubkey()],
                )
                .unwrap(),
            );
        }

        Ok(instructions)
    }
}
