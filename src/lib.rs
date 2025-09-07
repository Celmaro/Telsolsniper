pub mod common;
pub mod constants;
pub mod instruction;
pub mod protos;
pub mod swqos;
pub mod trading;
pub mod utils;
pub use solana_streamer_sdk;

use crate::constants::trade::trade::DEFAULT_SLIPPAGE;
use crate::swqos::SwqosConfig;
use crate::trading::core::params::BonkParams;
use crate::trading::core::params::PumpFunParams;
use crate::trading::core::params::PumpSwapParams;
use crate::trading::core::params::RaydiumAmmV4Params;
use crate::trading::core::params::RaydiumCpmmParams;
use crate::trading::core::traits::ProtocolParams;
use crate::trading::factory::DexType;
use crate::trading::BuyParams;
use crate::trading::MiddlewareManager;
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
    pub rpc_client: Vec<Arc<SwqosClient>>,
    pub swqos_clients: Vec<Arc<SwqosClient>>,
    pub priority_fee: PriorityFee,
    pub trade_config: TradeConfig,
    pub middleware_manager: Option<Arc<MiddlewareManager>>,
}

static INSTANCE: Mutex<Option<Arc<SolanaTrade>>> = Mutex::new(None);

impl Clone for SolanaTrade {
    fn clone(&self) -> Self {
        Self {
            payer: self.payer.clone(),
            rpc: self.rpc.clone(),
            rpc_client: self.rpc_client.clone(),
            swqos_clients: self.swqos_clients.clone(),
            priority_fee: self.priority_fee.clone(),
            trade_config: self.trade_config.clone(),
            middleware_manager: self.middleware_manager.clone(),
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

        let rpc = Arc::new(SolanaRpcClient::new_with_commitment(rpc_url.clone(), commitment));

        let rpc_client = SwqosConfig::get_swqos_client(
            rpc_url.clone(),
            commitment,
            SwqosConfig::Default(rpc_url),
        );

        let instance = Self {
            payer,
            rpc,
            rpc_client: vec![rpc_client],
            swqos_clients,
            priority_fee,
            trade_config: trade_config.clone(),
            middleware_manager: None,
        };

        let mut current = INSTANCE.lock().unwrap();
        *current = Some(Arc::new(instance.clone()));

        instance
    }

    pub fn with_middleware_manager(mut self, middleware_manager: MiddlewareManager) -> Self {
        self.middleware_manager = Some(Arc::new(middleware_manager));
        self
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
    /// * `dex_type` - The trading protocol to use (PumpFun, PumpSwap, or Bonk)
    /// * `mint` - The public key of the token mint to buy
    /// * `sol_amount` - Amount of SOL to spend on the purchase (in lamports)
    /// * `slippage_basis_points` - Optional slippage tolerance in basis points (e.g., 100 = 1%)
    /// * `recent_blockhash` - Recent blockhash for transaction validity
    /// * `custom_priority_fee` - Optional custom priority fee for priority processing
    /// * `extension_params` - Optional protocol-specific parameters (uses defaults if None)
    /// * `lookup_table_key` - Optional address lookup table key for transaction optimization
    /// * `wait_transaction_confirmed` - Whether to wait for the transaction to be confirmed
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
    pub async fn buy(
        &self,
        dex_type: DexType,
        mint: Pubkey,
        sol_amount: u64,
        slippage_basis_points: Option<u64>,
        recent_blockhash: Hash,
        custom_priority_fee: Option<PriorityFee>,
        extension_params: Box<dyn ProtocolParams>,
        lookup_table_key: Option<Pubkey>,
        wait_transaction_confirmed: bool,
    ) -> Result<(), anyhow::Error> {
        if slippage_basis_points.is_none() {
            println!(
                "slippage_basis_points is none, use default slippage basis points: {}",
                DEFAULT_SLIPPAGE
            );
        }
        let executor = TradeFactory::create_executor(dex_type.clone());
        let protocol_params = extension_params;

        let final_lookup_table_key = lookup_table_key.or(self.trade_config.lookup_table_key);

        let mut buy_params = BuyParams {
            rpc: Some(self.rpc.clone()),
            payer: self.payer.clone(),
            mint: mint,
            sol_amount: sol_amount,
            slippage_basis_points: slippage_basis_points,
            priority_fee: self.trade_config.priority_fee.clone(),
            lookup_table_key: final_lookup_table_key,
            recent_blockhash,
            data_size_limit: 0,
            wait_transaction_confirmed: wait_transaction_confirmed,
            protocol_params: protocol_params.clone(),
        };
        if custom_priority_fee.is_some() {
            buy_params.priority_fee = custom_priority_fee.unwrap();
        }

        // Validate protocol params
        let is_valid_params = match dex_type {
            DexType::PumpFun => protocol_params.as_any().downcast_ref::<PumpFunParams>().is_some(),
            DexType::PumpSwap => {
                protocol_params.as_any().downcast_ref::<PumpSwapParams>().is_some()
            }
            DexType::Bonk => protocol_params.as_any().downcast_ref::<BonkParams>().is_some(),
            DexType::RaydiumCpmm => {
                protocol_params.as_any().downcast_ref::<RaydiumCpmmParams>().is_some()
            }
            DexType::RaydiumAmmV4 => {
                protocol_params.as_any().downcast_ref::<RaydiumAmmV4Params>().is_some()
            }
        };

        if !is_valid_params {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }

        executor
            .buy_with_tip(buy_params, self.swqos_clients.clone(), self.middleware_manager.clone())
            .await
    }

    /// Execute a sell order for a specified token
    ///
    /// # Arguments
    ///
    /// * `dex_type` - The trading protocol to use (PumpFun, PumpSwap, or Bonk)
    /// * `mint` - The public key of the token mint to sell
    /// * `token_amount` - Amount of tokens to sell (in smallest token units)
    /// * `slippage_basis_points` - Optional slippage tolerance in basis points (e.g., 100 = 1%)
    /// * `recent_blockhash` - Recent blockhash for transaction validity
    /// * `custom_priority_fee` - Optional custom priority fee for priority processing
    /// * `with_tip` - Optional boolean to indicate if the transaction should be sent with tip
    /// * `extension_params` - Optional protocol-specific parameters (uses defaults if None)
    /// * `lookup_table_key` - Optional address lookup table key for transaction optimization
    /// * `wait_transaction_confirmed` - Whether to wait for the transaction to be confirmed
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
    pub async fn sell(
        &self,
        dex_type: DexType,
        mint: Pubkey,
        token_amount: u64,
        slippage_basis_points: Option<u64>,
        recent_blockhash: Hash,
        custom_priority_fee: Option<PriorityFee>,
        with_tip: bool,
        extension_params: Box<dyn ProtocolParams>,
        lookup_table_key: Option<Pubkey>,
        wait_transaction_confirmed: bool,
    ) -> Result<(), anyhow::Error> {
        if slippage_basis_points.is_none() {
            println!(
                "slippage_basis_points is none, use default slippage basis points: {}",
                DEFAULT_SLIPPAGE
            );
        }
        let executor = TradeFactory::create_executor(dex_type.clone());
        let protocol_params = extension_params;

        let final_lookup_table_key = lookup_table_key.or(self.trade_config.lookup_table_key);

        let mut sell_params = SellParams {
            rpc: Some(self.rpc.clone()),
            payer: self.payer.clone(),
            mint: mint,
            token_amount: Some(token_amount),
            slippage_basis_points: slippage_basis_points,
            priority_fee: self.trade_config.priority_fee.clone(),
            lookup_table_key: final_lookup_table_key,
            recent_blockhash,
            wait_transaction_confirmed: wait_transaction_confirmed,
            protocol_params: protocol_params.clone(),
            with_tip: with_tip,
        };
        if custom_priority_fee.is_some() {
            sell_params.priority_fee = custom_priority_fee.unwrap();
        }

        // Validate protocol params
        let is_valid_params = match dex_type {
            DexType::PumpFun => protocol_params.as_any().downcast_ref::<PumpFunParams>().is_some(),
            DexType::PumpSwap => {
                protocol_params.as_any().downcast_ref::<PumpSwapParams>().is_some()
            }
            DexType::Bonk => protocol_params.as_any().downcast_ref::<BonkParams>().is_some(),
            DexType::RaydiumCpmm => {
                protocol_params.as_any().downcast_ref::<RaydiumCpmmParams>().is_some()
            }
            DexType::RaydiumAmmV4 => {
                protocol_params.as_any().downcast_ref::<RaydiumAmmV4Params>().is_some()
            }
        };

        if !is_valid_params {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }

        let _swqos_clients =
            if !with_tip { self.rpc_client.clone() } else { self.swqos_clients.clone() };

        // Execute sell based on tip preference
        executor.sell_with_tip(sell_params, _swqos_clients, self.middleware_manager.clone()).await
    }

    /// Execute a sell order for a percentage of the specified token amount
    ///
    /// This is a convenience function that calculates the exact amount to sell based on
    /// a percentage of the total token amount and then calls the `sell` function.
    ///
    /// # Arguments
    ///
    /// * `dex_type` - The trading protocol to use (PumpFun, PumpSwap, or Bonk)
    /// * `mint` - The public key of the token mint to sell
    /// * `amount_token` - Total amount of tokens available (in smallest token units)
    /// * `percent` - Percentage of tokens to sell (1-100, where 100 = 100%)
    /// * `slippage_basis_points` - Optional slippage tolerance in basis points (e.g., 100 = 1%)
    /// * `recent_blockhash` - Recent blockhash for transaction validity
    /// * `custom_priority_fee` - Optional custom priority fee for priority processing
    /// * `with_tip` - Whether to use tip for priority processing
    /// * `extension_params` - Optional protocol-specific parameters (uses defaults if None)
    /// * `lookup_table_key` - Optional lookup table key for address lookup optimization
    /// * `wait_transaction_confirmed` - Whether to wait for the transaction to be confirmed
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
    pub async fn sell_by_percent(
        &self,
        dex_type: DexType,
        mint: Pubkey,
        amount_token: u64,
        percent: u64,
        slippage_basis_points: Option<u64>,
        recent_blockhash: Hash,
        custom_priority_fee: Option<PriorityFee>,
        with_tip: bool,
        extension_params: Box<dyn ProtocolParams>,
        lookup_table_key: Option<Pubkey>,
        wait_transaction_confirmed: bool,
    ) -> Result<(), anyhow::Error> {
        if percent == 0 || percent > 100 {
            return Err(anyhow::anyhow!("Percentage must be between 1 and 100"));
        }
        let amount = amount_token * percent / 100;
        self.sell(
            dex_type,
            mint,
            amount,
            slippage_basis_points,
            recent_blockhash,
            custom_priority_fee,
            with_tip,
            extension_params,
            lookup_table_key,
            wait_transaction_confirmed,
        )
        .await
    }
}
