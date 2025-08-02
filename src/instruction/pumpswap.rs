use anyhow::{anyhow, Result};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signer::Signer};
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::instruction::close_account;

use crate::{
    constants::{
        pumpswap::{accounts, BUY_DISCRIMINATOR, SELL_DISCRIMINATOR},
        trade::trade::DEFAULT_SLIPPAGE,
    },
    trading::{
        common::utils::{
            calculate_with_slippage_buy, calculate_with_slippage_sell, get_token_balance,
        },
        core::{
            params::{BuyParams, PumpSwapParams, SellParams},
            traits::InstructionBuilder,
        },
        pumpswap::{
            self,
            common::{
                coin_creator_vault_ata, coin_creator_vault_authority, fee_recipient_ata, find_pool, get_global_volume_accumulator_pda, get_token_amount, get_user_volume_accumulator_pda, get_wsol_amount
            },
        },
    },
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

        // Build instructions based on whether account information is provided
        match (&protocol_params.pool,) {
            (Some(pool),) => {
                let mut base_mint = params.mint;
                let mut quote_mint = accounts::WSOL_TOKEN_ACCOUNT;
                let mut pool_base_token_reserves = 0;
                let mut pool_quote_token_reserves = 0;

                if let Some(p_base_mint) = protocol_params.base_mint {
                    base_mint = p_base_mint;
                }
                if let Some(p_quote_mint) = protocol_params.quote_mint {
                    quote_mint = p_quote_mint;
                }
                if let Some(p_pool_base_token_reserves) = protocol_params.pool_base_token_reserves {
                    pool_base_token_reserves = p_pool_base_token_reserves;
                }
                if let Some(p_pool_quote_token_reserves) = protocol_params.pool_quote_token_reserves
                {
                    pool_quote_token_reserves = p_pool_quote_token_reserves;
                }

                self.build_buy_instructions_with_accounts(
                    params,
                    *pool,
                    base_mint,
                    quote_mint,
                    pool_base_token_reserves,
                    pool_quote_token_reserves,
                    protocol_params.auto_handle_wsol,
                )
                .await
            }
            _ => self.build_buy_instructions_auto_discover(params).await,
        }
    }

    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>> {
        // Get PumpSwap specific parameters
        let protocol_params = params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
            .ok_or_else(|| anyhow!("Invalid protocol params for PumpSwap"))?;
        // Build instructions based on whether account information is provided
        match (&protocol_params.pool,) {
            (Some(pool),) => {
                let mut base_mint = params.mint;
                let mut quote_mint = accounts::WSOL_TOKEN_ACCOUNT;
                let mut pool_base_token_reserves = 0;
                let mut pool_quote_token_reserves = 0;
                if let Some(p_base_mint) = protocol_params.base_mint {
                    base_mint = p_base_mint;
                }
                if let Some(p_quote_mint) = protocol_params.quote_mint {
                    quote_mint = p_quote_mint;
                }
                if let Some(p_pool_base_token_reserves) = protocol_params.pool_base_token_reserves {
                    pool_base_token_reserves = p_pool_base_token_reserves;
                }
                if let Some(p_pool_quote_token_reserves) = protocol_params.pool_quote_token_reserves
                {
                    pool_quote_token_reserves = p_pool_quote_token_reserves;
                }
                self.build_sell_instructions_with_accounts(
                    params,
                    *pool,
                    base_mint,
                    quote_mint,
                    pool_base_token_reserves,
                    pool_quote_token_reserves,
                    protocol_params.auto_handle_wsol,
                )
                .await
            }
            _ => self.build_sell_instructions_auto_discover(params).await,
        }
    }
}

impl PumpSwapInstructionBuilder {
    /// Auto-discover pool and account information and build buy instructions
    async fn build_buy_instructions_auto_discover(
        &self,
        params: &BuyParams,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        println!("❗️Going through RPC request, increasing instruction building time");
        let rpc = params.rpc.as_ref().unwrap().clone();
        // Find pool
        let pool = find_pool(rpc.as_ref(), &params.mint).await?;
        let pool_data = pumpswap::pool::Pool::fetch(rpc.as_ref(), &pool).await?;
        let pool_base_token_reserves =
            get_token_balance(rpc.as_ref(), &pool, &pool_data.base_mint).await?;
        let pool_quote_token_reserves =
            get_token_balance(rpc.as_ref(), &pool, &pool_data.quote_mint).await?;
        let mut params = params.clone();
        params.creator = pool_data.coin_creator;
        self.build_buy_instructions_with_accounts(
            &params,
            pool,
            pool_data.base_mint,
            pool_data.quote_mint,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            true,
        )
        .await
    }

    /// Auto-discover pool and account information and build sell instructions
    async fn build_sell_instructions_auto_discover(
        &self,
        params: &SellParams,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        println!("❗️Going through RPC request, increasing instruction building time");
        let rpc = params.rpc.as_ref().unwrap().clone();
        // Find pool
        let pool = find_pool(rpc.as_ref(), &params.mint).await?;
        let pool_data = pumpswap::pool::Pool::fetch(rpc.as_ref(), &pool).await?;
        let pool_base_token_reserves =
            get_token_balance(rpc.as_ref(), &pool, &pool_data.base_mint).await?;
        let pool_quote_token_reserves =
            get_token_balance(rpc.as_ref(), &pool, &pool_data.quote_mint).await?;
        let mut params = params.clone();
        params.creator = pool_data.coin_creator;
        self.build_sell_instructions_with_accounts(
            &params,
            pool,
            pool_data.base_mint,
            pool_data.quote_mint,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            true,
        )
        .await
    }

    /// Build buy instructions with provided account information
    async fn build_buy_instructions_with_accounts(
        &self,
        params: &BuyParams,
        pool: Pubkey,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        pool_base_token_reserves: u64,
        pool_quote_token_reserves: u64,
        auto_handle_wsol: bool,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let quote_mint_is_wsol = quote_mint == accounts::WSOL_TOKEN_ACCOUNT;
        // Calculate token amount
        let mut token_amount = get_token_amount(
            quote_mint_is_wsol,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            params.sol_amount,
            accounts::LP_FEE_BASIS_POINTS,
            accounts::PROTOCOL_FEE_BASIS_POINTS,
            if params.creator == Pubkey::default() {
                0
            } else {
                accounts::COIN_CREATOR_FEE_BASIS_POINTS
            },
        )
        .await?;
        if !quote_mint_is_wsol {
            // min_quote_amount_out
            token_amount = calculate_with_slippage_sell(
                token_amount,
                params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
            );
        }
        let sol_amount = if quote_mint_is_wsol {
            // max_quote_amount_in
            calculate_with_slippage_buy(
                params.sol_amount,
                params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
            )
        } else {
            // base_amount_in
            params.sol_amount
        };

        // Create user token accounts
        let user_base_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &base_mint,
        );
        let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &quote_mint,
        );

        // Get pool token accounts
        let pool_base_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &base_mint,
                &accounts::TOKEN_PROGRAM,
            );

        let pool_quote_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &quote_mint,
                &accounts::TOKEN_PROGRAM,
            );

        let mut instructions = vec![];

        if auto_handle_wsol {
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
                solana_sdk::system_instruction::transfer(
                    &params.payer.pubkey(),
                    if quote_mint_is_wsol {
                        &user_quote_token_account
                    } else {
                        &user_base_token_account
                    },
                    sol_amount,
                ),
            );

            // Sync wSOL balance
            instructions.push(
                spl_token::instruction::sync_native(
                    &accounts::TOKEN_PROGRAM,
                    if quote_mint_is_wsol {
                        &user_quote_token_account
                    } else {
                        &user_base_token_account
                    },
                )
                .unwrap(),
            );
        }

        // Create user's base token account
        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            if quote_mint_is_wsol {
                &base_mint
            } else {
                &quote_mint
            },
            &accounts::TOKEN_PROGRAM,
        ));

        let coin_creator_vault_ata = coin_creator_vault_ata(params.creator, quote_mint);
        let coin_creator_vault_authority = coin_creator_vault_authority(params.creator);
        let fee_recipient_ata = fee_recipient_ata(accounts::FEE_RECIPIENT, quote_mint);

        // Create buy instruction
        let mut accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(pool, false), // pool_id (readonly)
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // user (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_ACCOUNT, false), // global (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(base_mint, false), // base_mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(quote_mint, false), // quote_mint (readonly)
            solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // user_base_token_account
            solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
            solana_sdk::instruction::AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
            solana_sdk::instruction::AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::FEE_RECIPIENT, false), // fee_recipient (readonly)
            solana_sdk::instruction::AccountMeta::new(fee_recipient_ata, false), // fee_recipient_ata
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(
                accounts::ASSOCIATED_TOKEN_PROGRAM,
                false,
            ), // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // event_authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_PROGRAM, false), // PUMP_AMM_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new(coin_creator_vault_ata, false), // coin_creator_vault_ata
            solana_sdk::instruction::AccountMeta::new_readonly(coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
        ];
        if quote_mint_is_wsol {
            accounts.push(solana_sdk::instruction::AccountMeta::new(
                get_global_volume_accumulator_pda().unwrap(),
                false,
            ));
            accounts.push(solana_sdk::instruction::AccountMeta::new(
                get_user_volume_accumulator_pda(&params.payer.pubkey()).unwrap(),
                false,
            ));
        }

        // Create instruction data
        let mut data = vec![];
        if quote_mint_is_wsol {
            data.extend_from_slice(&BUY_DISCRIMINATOR);
            data.extend_from_slice(&token_amount.to_le_bytes());
            data.extend_from_slice(&sol_amount.to_le_bytes());
        } else {
            data.extend_from_slice(&SELL_DISCRIMINATOR);
            data.extend_from_slice(&sol_amount.to_le_bytes());
            data.extend_from_slice(&token_amount.to_le_bytes());
        }

        instructions.push(Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data,
        });
        if auto_handle_wsol {
            // Close wSOL ATA account, reclaim rent
            instructions.push(
                spl_token::instruction::close_account(
                    &accounts::TOKEN_PROGRAM,
                    if quote_mint_is_wsol {
                        &user_quote_token_account
                    } else {
                        &user_base_token_account
                    },
                    &params.payer.pubkey(),
                    &params.payer.pubkey(),
                    &[&params.payer.pubkey()],
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
        pool: Pubkey,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        pool_base_token_reserves: u64,
        pool_quote_token_reserves: u64,
        auto_handle_wsol: bool,
    ) -> Result<Vec<Instruction>> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }

        let quote_mint_is_wsol = quote_mint == accounts::WSOL_TOKEN_ACCOUNT;
        let mut sol_amount = get_wsol_amount(
            quote_mint_is_wsol,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            params.token_amount.unwrap_or(0),
            accounts::LP_FEE_BASIS_POINTS,
            accounts::PROTOCOL_FEE_BASIS_POINTS,
            if params.creator == Pubkey::default() {
                0
            } else {
                accounts::COIN_CREATOR_FEE_BASIS_POINTS
            },
        )
        .await?;
        sol_amount = calculate_with_slippage_sell(
            sol_amount,
            params.slippage_basis_points.unwrap_or(DEFAULT_SLIPPAGE),
        );
        let token_amount = params.token_amount.unwrap_or(0);

        let coin_creator_vault_ata = coin_creator_vault_ata(params.creator, quote_mint);
        let coin_creator_vault_authority = coin_creator_vault_authority(params.creator);
        let fee_recipient_ata = fee_recipient_ata(accounts::FEE_RECIPIENT, quote_mint);

        let user_base_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &base_mint,
        );
        let user_quote_token_account = spl_associated_token_account::get_associated_token_address(
            &params.payer.pubkey(),
            &quote_mint,
        );
        let pool_base_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &base_mint,
                &accounts::TOKEN_PROGRAM,
            );
        let pool_quote_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(
                &pool,
                &quote_mint,
                &accounts::TOKEN_PROGRAM,
            );

        let mut instructions = vec![];

        // Insert wSOL
        instructions.push(
            // Create wSOL ATA account if it doesn't exist
            create_associated_token_account_idempotent(
                &params.payer.pubkey(),
                &params.payer.pubkey(),
                &accounts::WSOL_TOKEN_ACCOUNT,
                &accounts::TOKEN_PROGRAM,
            ),
        );

        // Create user's token account
        instructions.push(create_associated_token_account_idempotent(
            &params.payer.pubkey(),
            &params.payer.pubkey(),
            if quote_mint_is_wsol {
                &base_mint
            } else {
                &quote_mint
            },
            &accounts::TOKEN_PROGRAM,
        ));

        // Create sell instruction
        let mut accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(pool, false), // pool_id (readonly)
            solana_sdk::instruction::AccountMeta::new(params.payer.pubkey(), true), // user (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::GLOBAL_ACCOUNT, false), // global (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(base_mint, false), // mint (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(quote_mint, false), // WSOL_TOKEN_ACCOUNT (readonly)
            solana_sdk::instruction::AccountMeta::new(user_base_token_account, false), // user_base_token_account
            solana_sdk::instruction::AccountMeta::new(user_quote_token_account, false), // user_quote_token_account
            solana_sdk::instruction::AccountMeta::new(pool_base_token_account, false), // pool_base_token_account
            solana_sdk::instruction::AccountMeta::new(pool_quote_token_account, false), // pool_quote_token_account
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::FEE_RECIPIENT, false), // fee_recipient (readonly)
            solana_sdk::instruction::AccountMeta::new(fee_recipient_ata, false), // fee_recipient_ata
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::TOKEN_PROGRAM, false), // TOKEN_PROGRAM_ID (readonly, duplicated as in JS)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::SYSTEM_PROGRAM, false), // System Program (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(
                accounts::ASSOCIATED_TOKEN_PROGRAM,
                false,
            ), // ASSOCIATED_TOKEN_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::EVENT_AUTHORITY, false), // event_authority (readonly)
            solana_sdk::instruction::AccountMeta::new_readonly(accounts::AMM_PROGRAM, false), // PUMP_AMM_PROGRAM_ID (readonly)
            solana_sdk::instruction::AccountMeta::new(coin_creator_vault_ata, false), // coin_creator_vault_ata
            solana_sdk::instruction::AccountMeta::new_readonly(coin_creator_vault_authority, false), // coin_creator_vault_authority (readonly)
        ];
        if !quote_mint_is_wsol {
            accounts.push(solana_sdk::instruction::AccountMeta::new(
                get_global_volume_accumulator_pda().unwrap(),
                false,
            ));
            accounts.push(solana_sdk::instruction::AccountMeta::new(
                get_user_volume_accumulator_pda(&params.payer.pubkey()).unwrap(),
                false,
            ));
        }

        // Create instruction data
        let mut data = vec![];
        if quote_mint_is_wsol {
            data.extend_from_slice(&SELL_DISCRIMINATOR);
            data.extend_from_slice(&token_amount.to_le_bytes());
            data.extend_from_slice(&sol_amount.to_le_bytes());
        } else {
            data.extend_from_slice(&BUY_DISCRIMINATOR);
            data.extend_from_slice(&sol_amount.to_le_bytes());
            data.extend_from_slice(&token_amount.to_le_bytes());
        }

        instructions.push(Instruction {
            program_id: accounts::AMM_PROGRAM,
            accounts,
            data,
        });

        if auto_handle_wsol {
            instructions.push(
                close_account(
                    &accounts::TOKEN_PROGRAM,
                    if quote_mint_is_wsol {
                        &user_quote_token_account
                    } else {
                        &user_base_token_account
                    },
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
