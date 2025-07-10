pub mod common;
pub mod constants;
pub mod instruction;
pub mod protos;
pub mod streaming;
pub mod swqos;
pub mod trading;
pub mod utils;

use crate::swqos::SwqosConfig;
use crate::trading::core::params::BonkParams;
use crate::trading::core::params::PumpFunParams;
use crate::trading::core::params::PumpSwapParams;
use crate::trading::core::traits::ProtocolParams;
use crate::trading::factory::DexType;
use crate::trading::BuyParams;
use crate::trading::SellParams;
use crate::trading::TradeFactory;
use common::{PriorityFee, SolanaRpcClient, TradeConfig};
use rustls::crypto::{ring::default_provider, CryptoProvider};
use solana_sdk::hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;
use std::sync::Mutex;
use swqos::SwqosClient;

pub struct SolanaTrade {
    pub payer: Arc<Keypair>,
    pub rpc: Arc<SolanaRpcClient>,
    pub swqos_clients: Vec<Arc<SwqosClient>>,
    pub priority_fee: PriorityFee,
    pub trade_config: TradeConfig,
}

static INSTANCE: Mutex<Option<Arc<SolanaTrade>>> = Mutex::new(None);

impl Clone for SolanaTrade {
    fn clone(&self) -> Self {
        Self {
            payer: self.payer.clone(),
            rpc: self.rpc.clone(),
            swqos_clients: self.swqos_clients.clone(),
            priority_fee: self.priority_fee.clone(),
            trade_config: self.trade_config.clone(),
        }
    }
}

impl SolanaTrade {
    #[inline]
    pub async fn new(payer: Arc<Keypair>, trade_config: TradeConfig) -> Self {
        if CryptoProvider::get_default().is_none() {
            let _ = default_provider()
                .install_default()
                .map_err(|e| anyhow::anyhow!("Failed to install crypto provider: {:?}", e));
        }

        let rpc_url = trade_config.rpc_url.clone();
        let swqos_configs = trade_config.swqos_configs.clone();
        let priority_fee = trade_config.priority_fee.clone();
        let commitment = trade_config.commitment.clone();

        let mut swqos_clients: Vec<Arc<SwqosClient>> = vec![];

        for swqos in swqos_configs {
            let swqos_client =
                SwqosConfig::get_swqos_client(rpc_url.clone(), commitment.clone(), swqos.clone());
            swqos_clients.push(swqos_client);
        }

        let rpc = Arc::new(SolanaRpcClient::new_with_commitment(
            rpc_url.clone(),
            commitment,
        ));

        let instance = Self {
            payer,
            rpc,
            swqos_clients,
            priority_fee,
            trade_config: trade_config.clone(),
        };

        let mut current = INSTANCE.lock().unwrap();
        *current = Some(Arc::new(instance.clone()));

        instance
    }

    /// Get the RPC client instance
    pub fn get_rpc(&self) -> &Arc<SolanaRpcClient> {
        &self.rpc
    }

    /// Get the current instance
    pub fn get_instance() -> Arc<Self> {
        let instance = INSTANCE.lock().unwrap();
        instance
            .as_ref()
            .expect("PumpFun instance not initialized. Please call new() first.")
            .clone()
    }

    /// Execute a buy order for a specified token
    ///
    /// # Arguments
    ///
    /// * `mint` - The public key of the token mint to buy
    /// * `creator` - Optional creator public key for the token (defaults to Pubkey::default() if None)
    /// * `amount_sol` - Amount of SOL to spend on the purchase (in lamports)
    /// * `slippage_basis_points` - Optional slippage tolerance in basis points (e.g., 100 = 1%)
    /// * `recent_blockhash` - Recent blockhash for transaction validity
    /// * `custom_buy_tip_fee` - Optional custom tip fee for priority processing (in SOL)
    /// * `with_tip` - Whether to include tip for MEV protection and priority processing
    /// * `protocol` - Trading protocol to use (PumpFun, PumpSwap, or Bonk)
    /// * `protocol_params` - Optional protocol-specific parameters (uses defaults if None)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the buy order is successfully executed, or an error if the transaction fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - Invalid protocol parameters are provided
    /// - The transaction fails to execute
    /// - Network or RPC errors occur
    /// - Insufficient SOL balance for the purchase
    ///
    /// # Example
    ///
    /// ```rust
    /// use solana_sdk::pubkey::Pubkey;
    /// use solana_sdk::hash::Hash;
    /// use crate::trading::factory::TradingProtocol;
    ///
    /// let mint = Pubkey::new_unique();
    /// let amount_sol = 1_000_000_000; // 1 SOL in lamports
    /// let slippage = Some(500); // 5% slippage
    /// let recent_blockhash = Hash::default();
    ///
    /// solana_trade.buy(
    ///     mint,
    ///     None,
    ///     amount_sol,
    ///     slippage,
    ///     recent_blockhash,
    ///     None,
    ///     true,
    ///     TradingProtocol::PumpFun,
    ///     None,
    /// ).await?;
    /// ```
    pub async fn buy(
        &self,
        dex_type: DexType,
        mint: Pubkey,
        creator: Option<Pubkey>,
        amount_sol: u64,
        slippage_basis_points: Option<u64>,
        recent_blockhash: Hash,
        custom_buy_tip_fee: Option<f64>,
        with_tip: bool,  
        extension_params: Option<Box<dyn ProtocolParams>>,
    ) -> Result<(), anyhow::Error> {
        let executor = TradeFactory::create_executor(dex_type.clone());
        let protocol_params = if let Some(params) = extension_params {
            params
        } else {
            match dex_type {
                DexType::PumpFun => {
                    Box::new(PumpFunParams::default()) as Box<dyn ProtocolParams>
                }
                DexType::PumpSwap => {
                    Box::new(PumpSwapParams::default()) as Box<dyn ProtocolParams>
                }
                DexType::Bonk => Box::new(BonkParams::default()) as Box<dyn ProtocolParams>,
            }
        };
        let buy_params = BuyParams {
            rpc: Some(self.rpc.clone()),
            payer: self.payer.clone(),
            mint: mint,
            creator: creator.unwrap_or(Pubkey::default()),
            amount_sol: amount_sol,
            slippage_basis_points: slippage_basis_points,
            priority_fee: self.trade_config.priority_fee.clone(),
            lookup_table_key: self.trade_config.lookup_table_key,
            recent_blockhash,
            data_size_limit: 0,
            protocol_params: protocol_params.clone(),
        };
        let mut priority_fee = buy_params.priority_fee.clone();
        if custom_buy_tip_fee.is_some() {
            priority_fee.buy_tip_fee = custom_buy_tip_fee.unwrap();
            priority_fee.buy_tip_fees = vec![
                custom_buy_tip_fee.unwrap(),
                custom_buy_tip_fee.unwrap(),
                custom_buy_tip_fee.unwrap(),
                custom_buy_tip_fee.unwrap(),
            ];
        }
        let buy_with_tip_params = buy_params.clone().with_tip(self.swqos_clients.clone());

        // Validate protocol params
        let is_valid_params = match dex_type {
            DexType::PumpFun => protocol_params
                .as_any()
                .downcast_ref::<PumpFunParams>()
                .is_some(),
            DexType::PumpSwap => protocol_params
                .as_any()
                .downcast_ref::<PumpSwapParams>()
                .is_some(),
            DexType::Bonk => protocol_params
                .as_any()
                .downcast_ref::<BonkParams>()
                .is_some(),
        };

        if !is_valid_params {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }

        // Execute buy based on tip preference
        if with_tip {
            executor.buy_with_tip(buy_with_tip_params).await
        } else {
            executor.buy(buy_params).await
        }
    }

    /// Execute a sell order for a specified token
    ///
    /// # Arguments
    ///
    /// * `mint` - The public key of the token mint to sell
    /// * `creator` - Optional creator public key for the token (defaults to Pubkey::default() if None)
    /// * `amount_token` - Amount of tokens to sell (in smallest token units)
    /// * `slippage_basis_points` - Optional slippage tolerance in basis points (e.g., 100 = 1%)
    /// * `recent_blockhash` - Recent blockhash for transaction validity
    /// * `custom_buy_tip_fee` - Optional custom tip fee for priority processing (in SOL)
    /// * `with_tip` - Whether to include tip for MEV protection and priority processing
    /// * `protocol` - Trading protocol to use (PumpFun, PumpSwap, or Bonk)
    /// * `protocol_params` - Optional protocol-specific parameters (uses defaults if None)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the sell order is successfully executed, or an error if the transaction fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - Invalid protocol parameters are provided
    /// - The transaction fails to execute
    /// - Network or RPC errors occur
    /// - Insufficient token balance for the sale
    /// - Token account doesn't exist or is not properly initialized
    ///
    /// # Example
    ///
    /// ```rust
    /// use solana_sdk::pubkey::Pubkey;
    /// use solana_sdk::hash::Hash;
    /// use crate::trading::factory::DexType;
    ///
    /// let mint = Pubkey::new_unique();
    /// let amount_token = 1_000_000; // Amount of tokens to sell
    /// let slippage = Some(500); // 5% slippage
    /// let recent_blockhash = Hash::default();
    ///
    /// solana_trade.sell(
    ///     mint,
    ///     None,
    ///     amount_token,
    ///     slippage,
    ///     recent_blockhash,
    ///     None,
    ///     true,
    ///     DexType::PumpFun,
    ///     None,
    /// ).await?;
    /// ```
    pub async fn sell(
        &self,
        dex_type: DexType,
        mint: Pubkey,
        creator: Option<Pubkey>,
        amount_token: u64,
        slippage_basis_points: Option<u64>,
        recent_blockhash: Hash,
        custom_buy_tip_fee: Option<f64>,
        with_tip: bool,
        extension_params: Option<Box<dyn ProtocolParams>>,
    ) -> Result<(), anyhow::Error> {
        let executor = TradeFactory::create_executor(dex_type.clone());
        let protocol_params = if let Some(params) = extension_params {
            params
        } else {
            match dex_type {
                DexType::PumpFun => {
                    Box::new(PumpFunParams::default()) as Box<dyn ProtocolParams>
                }
                DexType::PumpSwap => {
                    Box::new(PumpSwapParams::default()) as Box<dyn ProtocolParams>
                }
                DexType::Bonk => Box::new(BonkParams::default()) as Box<dyn ProtocolParams>,
            }
        };
        let sell_params = SellParams {
            rpc: Some(self.rpc.clone()),
            payer: self.payer.clone(),
            mint: mint,
            creator: creator.unwrap_or(Pubkey::default()),
            amount_token: Some(amount_token),
            slippage_basis_points: slippage_basis_points,
            priority_fee: self.trade_config.priority_fee.clone(),
            lookup_table_key: self.trade_config.lookup_table_key,
            recent_blockhash,
            protocol_params: protocol_params.clone(),
        };
        let mut priority_fee = sell_params.priority_fee.clone();
        if custom_buy_tip_fee.is_some() {
            priority_fee.buy_tip_fee = custom_buy_tip_fee.unwrap();
            priority_fee.buy_tip_fees = vec![
                custom_buy_tip_fee.unwrap(),
                custom_buy_tip_fee.unwrap(),
                custom_buy_tip_fee.unwrap(),
                custom_buy_tip_fee.unwrap(),
            ];
        }
        let sell_with_tip_params = sell_params.clone().with_tip(self.swqos_clients.clone());

        // Validate protocol params
        let is_valid_params = match dex_type {
            DexType::PumpFun => protocol_params
                .as_any()
                .downcast_ref::<PumpFunParams>()
                .is_some(),
            DexType::PumpSwap => protocol_params
                .as_any()
                .downcast_ref::<PumpSwapParams>()
                .is_some(),
            DexType::Bonk => protocol_params
                .as_any()
                .downcast_ref::<BonkParams>()
                .is_some(),
        };

        if !is_valid_params {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }

        // Execute sell based on tip preference
        if with_tip {
            executor.sell_with_tip(sell_with_tip_params).await
        } else {
            executor.sell(sell_params).await
        }
    }

    /// Execute a sell order for a percentage of the specified token amount
    ///
    /// This is a convenience function that calculates the exact amount to sell based on
    /// a percentage of the total token amount and then calls the `sell` function.
    ///
    /// # Arguments
    ///
    /// * `mint` - The public key of the token mint to sell
    /// * `creator` - Optional creator public key for the token (defaults to Pubkey::default() if None)
    /// * `amount_token` - Total amount of tokens available (in smallest token units)
    /// * `percent` - Percentage of tokens to sell (1-100, where 100 = 100%)
    /// * `slippage_basis_points` - Optional slippage tolerance in basis points (e.g., 100 = 1%)
    /// * `recent_blockhash` - Recent blockhash for transaction validity
    /// * `custom_buy_tip_fee` - Optional custom tip fee for priority processing (in SOL)
    /// * `with_tip` - Whether to include tip for MEV protection and priority processing
    /// * `protocol` - Trading protocol to use (PumpFun, PumpSwap, or Bonk)
    /// * `protocol_params` - Optional protocol-specific parameters (uses defaults if None)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the sell order is successfully executed, or an error if the transaction fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - `percent` is 0 or greater than 100
    /// - Invalid protocol parameters are provided
    /// - The transaction fails to execute
    /// - Network or RPC errors occur
    /// - Insufficient token balance for the calculated sale amount
    /// - Token account doesn't exist or is not properly initialized
    ///
    /// # Example
    ///
    /// ```rust
    /// use solana_sdk::pubkey::Pubkey;
    /// use solana_sdk::hash::Hash;
    /// use crate::trading::factory::TradingProtocol;
    ///
    /// let mint = Pubkey::new_unique();
    /// let total_tokens = 10_000_000; // Total tokens available
    /// let percent = 50; // Sell 50% of tokens
    /// let slippage = Some(500); // 5% slippage
    /// let recent_blockhash = Hash::default();
    ///
    /// // This will sell 5_000_000 tokens (50% of 10_000_000)
    /// solana_trade.sell_by_percent(
    ///     mint,
    ///     None,
    ///     total_tokens,
    ///     percent,
    ///     slippage,
    ///     recent_blockhash,
    ///     None,
    ///     true,
    ///     DexType::PumpFun,
    ///     None,
    /// ).await?;
    /// ```
    pub async fn sell_by_percent(
        &self,
        dex_type: DexType,
        mint: Pubkey,
        creator: Option<Pubkey>,
        amount_token: u64,
        percent: u64,
        slippage_basis_points: Option<u64>,
        recent_blockhash: Hash,
        custom_buy_tip_fee: Option<f64>,
        with_tip: bool,
        extension_params: Option<Box<dyn ProtocolParams>>,
    ) -> Result<(), anyhow::Error> {
        if percent == 0 || percent > 100 {
            return Err(anyhow::anyhow!("Percentage must be between 1 and 100"));
        }
        let amount = amount_token * percent / 100;
        self.sell(
            dex_type,
            mint,
            creator,
            amount,
            slippage_basis_points,
            recent_blockhash,
            custom_buy_tip_fee,
            with_tip,
            extension_params,
        )
        .await
    }
}
