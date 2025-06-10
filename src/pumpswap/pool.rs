use solana_sdk::pubkey::Pubkey;
use anyhow::anyhow;
use solana_account_decoder::UiAccountEncoding;
use crate::{common::SolanaRpcClient, constants::pumpswap::accounts};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Pool {
    pub pool_bump: u8,
    pub index: u16,
    pub creator: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub lp_supply: u64,
}

impl Pool {
    pub fn from_bytes(data: &[u8]) -> Result<Self, anyhow::Error> {
        if data.len() < 211 {
            return Err(anyhow!("Data too short for Pool account"));
        }

        // 跳过discriminator (8字节)
        let data = &data[8..];

        let pool_bump = data[0];
        let index = u16::from_le_bytes([data[1], data[2]]);

        let creator = Pubkey::new_from_array(data[3..35].try_into().map_err(|e| anyhow!("Failed to convert creator: {:?}", e))?);
        let base_mint = Pubkey::new_from_array(data[35..67].try_into().map_err(|e| anyhow!("Failed to convert base_mint: {:?}", e))?);
        let quote_mint = Pubkey::new_from_array(data[67..99].try_into().map_err(|e| anyhow!("Failed to convert quote_mint: {:?}", e))?);
        let lp_mint = Pubkey::new_from_array(data[99..131].try_into().map_err(|e| anyhow!("Failed to convert lp_mint: {:?}", e))?);
        let pool_base_token_account = Pubkey::new_from_array(data[131..163].try_into().map_err(|e| anyhow!("Failed to convert pool_base_token_account: {:?}", e))?);
        let pool_quote_token_account = Pubkey::new_from_array(data[163..195].try_into().map_err(|e| anyhow!("Failed to convert pool_quote_token_account: {:?}", e))?);

        let lp_supply = u64::from_le_bytes([
            data[195], data[196], data[197], data[198],
            data[199], data[200], data[201], data[202],
        ]);

        Ok(Self {
            pool_bump,
            index,
            creator,
            base_mint,
            quote_mint,
            lp_mint,
            pool_base_token_account,
            pool_quote_token_account,
            lp_supply,
        })
    }

    pub async fn fetch(
        rpc: &SolanaRpcClient,
        pool_address: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let account = rpc.get_account(pool_address).await?;

        if account.owner != accounts::AMM_PROGRAM {
            return Err(anyhow!("Account is not owned by PumpSwap program"));
        }

        Self::from_bytes(&account.data)
    }

    pub async fn find_by_mint(
        rpc: &SolanaRpcClient,
        mint: &Pubkey,
    ) -> Result<(Pubkey, Self), anyhow::Error> {
        // 使用getProgramAccounts查找给定mint的池子
        let filters = vec![
            solana_rpc_client_api::filter::RpcFilterType::DataSize(211), // Pool账户的大小
            solana_rpc_client_api::filter::RpcFilterType::Memcmp(
                solana_client::rpc_filter::Memcmp::new_base58_encoded(43, &mint.to_bytes()),
            ),
        ];

        let config = solana_rpc_client_api::config::RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: solana_rpc_client_api::config::RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: None,
                commitment: None,
                min_context_slot: None,
            },
            with_context: None,
            sort_results: None,
        };

        let program_id = crate::constants::pumpswap::accounts::AMM_PROGRAM;
        println!("program_id: {:?}", program_id);
        let accounts = rpc.get_program_accounts_with_config(&program_id, config).await?;

        if accounts.is_empty() {
            return Err(anyhow!("No pool found for mint {}", mint));
        }

        let mut pools: Vec<_> = accounts.into_iter()
            .filter_map(|(addr, acc)| {
                Self::from_bytes(&acc.data)
                    .map(|pool| (addr, pool))
                    .ok()
            })
            .collect();
        pools.sort_by(|a, b| b.1.lp_supply.cmp(&a.1.lp_supply));

        let (address, pool) = pools[0].clone();
        println!("pool: {:?}", pool);
        println!("address: {:?}", address);
        Ok((address, pool))
    }

    pub async fn get_token_balances(
        &self,
        rpc: &SolanaRpcClient,
    ) -> Result<(u64, u64), anyhow::Error> {
        let base_balance = rpc.get_token_account_balance(&self.pool_base_token_account).await?;
        let quote_balance = rpc.get_token_account_balance(&self.pool_quote_token_account).await?;

        let base_amount = base_balance.amount.parse::<u64>().map_err(|e| anyhow!(e))?;
        let quote_amount = quote_balance.amount.parse::<u64>().map_err(|e| anyhow!(e))?;

        Ok((base_amount, quote_amount))
    }

    pub async fn calculate_buy_amount(
        &self,
        rpc: &SolanaRpcClient,
        sol_amount: u64,
    ) -> Result<u64, anyhow::Error> {
        let (base_amount, quote_amount) = self.get_token_balances(rpc).await?;

        // 使用常数乘积公式 (x * y = k) 计算
        let product = base_amount as u128 * quote_amount as u128;
        let new_quote_amount = quote_amount as u128 + sol_amount as u128;
        let new_base_amount = product / new_quote_amount;

        let token_amount = base_amount as u128 - new_base_amount;

        Ok(token_amount as u64)
    }

    pub async fn calculate_sell_amount(
        &self,
        rpc: &SolanaRpcClient,
        token_amount: u64,
    ) -> Result<u64, anyhow::Error> {
        let (base_amount, quote_amount) = self.get_token_balances(rpc).await?;

        // 使用常数乘积公式 (x * y = k) 计算
        let product = base_amount as u128 * quote_amount as u128;
        let new_base_amount = base_amount as u128 + token_amount as u128;
        let new_quote_amount = product / new_base_amount;

        let sol_amount = quote_amount as u128 - new_quote_amount;

        Ok(sol_amount as u64)
    }
}
