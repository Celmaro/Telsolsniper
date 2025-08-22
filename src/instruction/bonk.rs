use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, signer::Signer};
use solana_system_interface::instruction::transfer;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::instruction::close_account;

use crate::{
    constants::{
        bonk::{accounts, BUY_EXECT_IN_DISCRIMINATOR, SELL_EXECT_IN_DISCRIMINATOR},
        trade::trade::DEFAULT_SLIPPAGE,
    },
    trading::{
        bonk::common::{get_pool_pda, get_vault_pda},
        common::utils::get_token_balance,
        core::{
            params::{BonkParams, BuyParams, SellParams},
            traits::InstructionBuilder,
        },
    },
    utils::calc::bonk::{
        get_buy_token_amount_from_sol_amount, get_sell_sol_amount_from_token_amount,
    },
};

/// Instruction builder for Bonk protocol
pub struct BonkInstructionBuilder;

#[async_trait::async_trait]
impl InstructionBuilder for BonkInstructionBuilder {
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

impl BonkInstructionBuilder {
    /// Build buy instructions with provided account information
    async fn build_buy_instructions_with_accounts(
        &self,
        params: &BuyParams,
    ) -> Result<Vec<Instruction>> {
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for Bonk"))?;

        let pool_state = get_pool_pda(&params.mint, &accounts::WSOL_TOKEN_ACCOUNT).unwrap();

        // Create user token accounts
        let user_base_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );
        let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );

        // Get pool token accounts
        let base_vault_account = get_vault_pda(&pool_state, &params.mint).unwrap();
        let quote_vault_account =
            get_vault_pda(&pool_state, &accounts::WSOL_TOKEN_ACCOUNT).unwrap();

        let virtual_base = protocol_params.virtual_base;
        let virtual_quote = protocol_params.virtual_quote;
        let real_base = protocol_params.real_base;
        let real_quote = protocol_params.real_quote;

        let amount_in: u64 = params.sol_amount;
        let share_fee_rate: u64 = 0;
        let minimum_amount_out: u64 = get_buy_token_amount_from_sol_amount(
            amount_in,
            virtual_base,
            virtual_quote,
            real_base,
            real_quote,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE) as u128,
        );

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
                transfer(&params.payer.pubkey(), &user_quote_token_account, amount_in),
            );

            // Sync wSOL balance
            instructions.push(
                spl_token::instruction::sync_native(
                    &accounts::TOKEN_PROGRAM,
                    &user_quote_token_account,
                )
                .unwrap(),
            );
        }

        // Create user's base token account
        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            &params.mint,
            &accounts::TOKEN_PROGRAM,
        ));

        // Create buy instruction
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // Payer (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AUTHORITY, false), // Authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_CONFIG, false), // Global Config (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(protocol_params.platform_onfig, false), // Platform Config (readonly)
            solana_sdk::instruction::AccountMeta::new(pool_state, false), // Pool State
            solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // User Base Token
            solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // User Quote Token
            solana_sdk::instruction::AccountMeta::new(base_vault_account, false), // Base Vault
            solana_sdk::instruction::AccountMeta::new(quote_vault_account, false), // Quote Vault
            solana_sdk::instruction::AccountMeta::new_readonly(params.mint, false), // Base Token Mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // Quote Token Mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(
                protocol_params.mint_token_program,
                false,
            ), // Base Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // Quote Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // Event Authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::BONK, false), // Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
            solana_sdk::instruction::AccountMeta::new(protocol_params.platform_associated_account, false), // Platform Associated Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.creator_associated_account, false), // Creator Associated Account
        ];
        // Create instruction data
        let mut data = vec![];
        data.extend_from_slice(&BUY_EXECT_IN_DISCRIMINATOR);
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        data.extend_from_slice(&share_fee_rate.to_le_bytes());

        instructions.push(Instruction { program_id: accounts::BONK, accounts, data });

        if protocol_params.auto_handle_wsol {
            // Close wSOL ATA account, reclaim rent
            instructions.push(
                spl_token::instruction::close_account(
                    &accounts::TOKEN_PROGRAM,
                    &user_quote_token_account,
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
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }

        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for Bonk"))?;

        let rpc = params.rpc.as_ref().unwrap().clone();

        // Get token balance
        let mut amount = params.token_amount;
        if params.token_amount.is_none() || params.token_amount.unwrap_or(0) == 0 {
            let balance_u64 =
                get_token_balance(rpc.as_ref(), &params.payer.pubkey(), &params.mint).await?;
            amount = Some(balance_u64);
        }
        let amount = amount.unwrap_or(0);

        if amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        let pool_state = get_pool_pda(&params.mint, &accounts::WSOL_TOKEN_ACCOUNT).unwrap();

        let virtual_base = protocol_params.virtual_base;
        let virtual_quote = protocol_params.virtual_quote;
        let real_base = protocol_params.real_base;
        let real_quote = protocol_params.real_quote;

        // Calculate expected SOL amount
        let minimum_amount_out: u64 = get_sell_sol_amount_from_token_amount(
            amount,
            virtual_base,
            virtual_quote,
            real_base,
            real_quote,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE) as u128,
        );

        // Create user token accounts
        let user_base_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &params.mint,
        );
        let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &accounts::WSOL_TOKEN_ACCOUNT,
        );

        // Get pool token accounts
        let base_vault_account = get_vault_pda(&pool_state, &params.mint).unwrap();
        let quote_vault_account =
            get_vault_pda(&pool_state, &accounts::WSOL_TOKEN_ACCOUNT).unwrap();

        let share_fee_rate: u64 = 0;

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

        // Create sell instruction
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // Payer (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AUTHORITY, false), // Authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_CONFIG, false), // Global Config (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(protocol_params.platform_onfig, false), // Platform Config (readonly)
            solana_sdk::instruction::AccountMeta::new(pool_state, false), // Pool State
            solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // User Base Token
            solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // User Quote Token
            solana_sdk::instruction::AccountMeta::new(base_vault_account, false), // Base Vault
            solana_sdk::instruction::AccountMeta::new(quote_vault_account, false), // Quote Vault
            solana_sdk::instruction::AccountMeta::new_readonly(params.mint, false), // Base Token Mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::WSOL_TOKEN_ACCOUNT, false), // Quote Token Mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(
                protocol_params.mint_token_program,
                false,
            ), // Base Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // Quote Token Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // Event Authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::BONK, false), // Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
            solana_sdk::instruction::AccountMeta::new(protocol_params.platform_associated_account, false), // Platform Associated Account
            solana_sdk::instruction::AccountMeta::new(protocol_params.creator_associated_account, false), // Creator Associated Account
        ];

        // Create instruction data
        let mut data = vec![];
        data.extend_from_slice(&SELL_EXECT_IN_DISCRIMINATOR);
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        data.extend_from_slice(&share_fee_rate.to_le_bytes());

        instructions.push(Instruction { program_id: accounts::BONK, accounts, data });

        if protocol_params.auto_handle_wsol {
            instructions.push(
                close_account(
                    &accounts::TOKEN_PROGRAM,
                    &user_quote_token_account,
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
