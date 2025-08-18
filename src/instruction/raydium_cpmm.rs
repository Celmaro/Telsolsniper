use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, signer::Signer};
use solana_system_interface::instruction::transfer;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::instruction::close_account;

use crate::{
    constants::{
        raydium_cpmm::{accounts, SWAP_BASE_IN_DISCRIMINATOR},
        trade::trade::DEFAULT_SLIPPAGE,
    },
    trading::{
        core::{
            params::{BuyParams, RaydiumCpmmParams, SellParams},
            traits::InstructionBuilder,
        },
        raydium_cpmm::common::{get_observation_state_pda, get_pool_pda, get_vault_pda},
    },
    utils::calc::raydium_cpmm::compute_swap_amount,
};

/// Instruction builder for RaydiumCpmm protocol
pub struct RaydiumCpmmInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for RaydiumCpmmInstructionBuilder {
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

impl RaydiumCpmmInstructionBuilder {
    /// Build buy instructions with provided account information
    async fn build_buy_instructions_with_accounts(
        &self,
        params: &BuyParams,
    ) -> Result<Vec<Instruction>> {
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<RaydiumCpmmParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for RaydiumCpmm"))?;

        let pool_state = get_pool_pda(
            &accounts::AMM_CONFIG,
            &protocol_params.base_mint,
            &protocol_params.quote_mint,
        )
        .unwrap();

        let wsol_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );
        let mint_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );

        // Get pool token accounts
        let wsol_vault_account = get_vault_pda(&pool_state, &accounts::WSOL_TOKEN_ACCOUNT).unwrap();
        let mint_vault_account = get_vault_pda(&pool_state, &params.mint).unwrap();

        let observation_state_account = get_observation_state_pda(&pool_state).unwrap();
        let is_base_in = protocol_params.base_mint == accounts::WSOL_TOKEN_ACCOUNT;

        let amount_in: u64 = params.sol_amount;
        let result = compute_swap_amount(
            protocol_params.base_reserve,
            protocol_params.quote_reserve,
            is_base_in,
            amount_in,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let minimum_amount_out = result.min_amount_out;

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

        let mint_token_program = if is_base_in {
            protocol_params.quote_token_program
        } else {
            protocol_params.base_token_program
        };

        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &mint_token_program,
        ));

        // Create buy instruction
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // Payer (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AUTHORITY, false), // Authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_CONFIG, false), // Amm Config (readonly)
            solana_sdk::instruction::AccountMeta::new(pool_state, false), // Pool State
            solana_sdk::instruction::AccountMeta::new(wsol_token_account, false), // Input Token Account
            solana_sdk::instruction::AccountMeta::new(mint_token_account, false), // Output Token Account
            solana_sdk::instruction::AccountMeta::new(wsol_vault_account, false), // Input Vault Account
            solana_sdk::instruction::AccountMeta::new(mint_vault_account, false), // Output Vault Account
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // Input Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(mint_token_program, false), // Output Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // Input token mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(params.mint, false), // Output token mint (readonly)
            solana_sdk::instruction::AccountMeta::new(observation_state_account, false), // Observation State Account
        ];
        // Create instruction data
        let mut data = vec![];
        data.extend_from_slice(&SWAP_BASE_IN_DISCRIMINATOR);
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());

        instructions.push(Instruction { program_id: accounts::RAYDIUM_CPMM, accounts, data });

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
            .downcast_ref::<RaydiumCpmmParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for RaydiumCpmm"))?;

        if params.token_amount.is_none() || params.token_amount.unwrap_or(0) == 0 {
            return Err(anyhow!("Token amount is not set"));
        }

        let is_base_in = protocol_params.base_mint == params.mint;

        let minimum_amount_out: u64 = compute_swap_amount(
            protocol_params.base_reserve,
            protocol_params.quote_reserve,
            is_base_in,
            params.token_amount.unwrap_or(0),
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        )
        .min_amount_out;

        let pool_state = get_pool_pda(
            &accounts::AMM_CONFIG,
            &protocol_params.base_mint,
            &protocol_params.quote_mint,
        )
        .unwrap();

        let wsol_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );
        let mint_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );

        // Get pool token accounts
        let wsol_vault_account = get_vault_pda(&pool_state, &accounts::WSOL_TOKEN_ACCOUNT).unwrap();
        let mint_vault_account = get_vault_pda(&pool_state, &params.mint).unwrap();

        let observation_state_account = get_observation_state_pda(&pool_state).unwrap();

        let mut instructions = vec![];

        // Handle wSOL
        instructions.push(
            // Create wSOL ATA account if it doesn't exist
            create_associated_token_account_idempotent(
                &params.payer.pubkey(),
                &params.payer.pubkey(),
                &accounts::WSOL_TOKEN_ACCOUNT,
                &spl_token::ID,
            ),
        );

        let mint_token_program = if is_base_in {
            protocol_params.base_token_program
        } else {
            protocol_params.quote_token_program
        };

        // Create sell instruction
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // Payer (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AUTHORITY, false), // Authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_CONFIG, false), // Amm Config (readonly)
            solana_sdk::instruction::AccountMeta::new(pool_state, false), // Pool State
            solana_sdk::instruction::AccountMeta::new(mint_token_account, false), // Input Token Account
            solana_sdk::instruction::AccountMeta::new(wsol_token_account, false), // Output Token Account
            solana_sdk::instruction::AccountMeta::new(mint_vault_account, false), // Input Vault Account
            solana_sdk::instruction::AccountMeta::new(wsol_vault_account, false), // Output Vault Account
            solana_sdk::instruction::AccountMeta::new_readonly(mint_token_program, false), // Input Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // Output Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(params.mint, false), // Input token mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // Output token mint (readonly)
            solana_sdk::instruction::AccountMeta::new(observation_state_account, false), // Observation State Account
        ];
        // Create instruction data
        let mut data = vec![];
        data.extend_from_slice(&SWAP_BASE_IN_DISCRIMINATOR);
        data.extend_from_slice(&params.token_amount.unwrap_or(0).to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());

        instructions.push(Instruction { program_id: accounts::RAYDIUM_CPMM, accounts, data });

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
