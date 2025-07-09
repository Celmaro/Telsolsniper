pub mod accounts;
pub mod common;
pub mod constants;
pub mod error;
pub mod event_parser;
pub mod grpc;
pub mod instruction;
pub mod protos;
pub mod swqos;
pub mod trading;

use std::sync::Arc;
use std::sync::Mutex;

use rustls::crypto::{ring::default_provider, CryptoProvider};
use solana_hash::Hash;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use swqos::SwqosClient;

use common::{PriorityFee, SolanaRpcClient, TradeConfig};

use accounts::BondingCurveAccount;
use constants::trade_platform::{PUMPFUN, PUMPFUN_SWAP, BONK};
use constants::trade_type::{COPY_BUY, SNIPER_BUY};

use crate::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use crate::swqos::SwqosConfig;
use crate::trading::core::params::PumpFunParams;
use crate::trading::core::params::PumpFunSellParams;
use crate::trading::core::params::PumpSwapParams;
use crate::trading::core::params::BonkParams;
use crate::trading::BuyWithTipParams;
use crate::trading::SellParams;
use crate::trading::SellWithTipParams;

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

    pub async fn buy_use_buy_params(
        &self,
        buy_params: BuyWithTipParams,
        custom_buy_tip_fee: Option<f64>,
    ) -> Result<(), anyhow::Error> {
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
        let mint = buy_params.mint;
        let creator = buy_params.creator;
        let buy_sol_cost = buy_params.amount_sol;
        let slippage_basis_points = buy_params.slippage_basis_points;
        let recent_blockhash = buy_params.recent_blockhash;
        if let Some(protocol_params) = buy_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunParams>()
        {
            trading::pumpfun::buy::buy(
                self.rpc.clone(),
                self.payer.clone(),
                mint,
                creator,
                buy_sol_cost,
                slippage_basis_points,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.bonding_curve.clone(),
                COPY_BUY.to_string(),
            )
            .await
        } else if let Some(protocol_params) = buy_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
        {
            trading::pumpswap::buy::buy(
                self.rpc.clone(),
                self.payer.clone(),
                mint,
                creator,
                buy_sol_cost,
                slippage_basis_points,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.pool.clone(),
                protocol_params.pool_base_token_account.clone(),
                protocol_params.pool_quote_token_account.clone(),
                protocol_params.user_base_token_account.clone(),
                protocol_params.user_quote_token_account.clone(),
                protocol_params.auto_handle_wsol,
            )
            .await
        } else if let Some(protocol_params) = buy_params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
        {
            trading::bonk::buy::buy(
                self.rpc.clone(),
                self.payer.clone(),
                mint,
                protocol_params.virtual_base.unwrap_or(0),
                protocol_params.virtual_quote.unwrap_or(0),
                protocol_params.real_base_before.unwrap_or(0),
                protocol_params.real_quote_before.unwrap_or(0),
                buy_sol_cost,
                slippage_basis_points,
                priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.auto_handle_wsol,
            )
            .await
        } else {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }
    }

    pub async fn buy_with_tip_use_buy_params(
        &self,
        buy_params: BuyWithTipParams,
        custom_buy_tip_fee: Option<f64>,
    ) -> Result<(), anyhow::Error> {
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
        let mint = buy_params.mint;
        let creator = buy_params.creator;
        let buy_sol_cost = buy_params.amount_sol;
        let slippage_basis_points = buy_params.slippage_basis_points;
        let recent_blockhash = buy_params.recent_blockhash;
        if let Some(protocol_params) = buy_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunParams>()
        {
            trading::pumpfun::buy::buy_with_tip(
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                creator,
                buy_sol_cost,
                slippage_basis_points,
                priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.bonding_curve.clone(),
                COPY_BUY.to_string(),
            )
            .await
        } else if let Some(protocol_params) = buy_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
        {
            trading::pumpswap::buy::buy_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                creator,
                buy_sol_cost,
                slippage_basis_points,
                priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.pool.clone(),
                protocol_params.pool_base_token_account.clone(),
                protocol_params.pool_quote_token_account.clone(),
                protocol_params.user_base_token_account.clone(),
                protocol_params.user_quote_token_account.clone(),
                protocol_params.auto_handle_wsol,
            )
            .await
        } else if let Some(protocol_params) = buy_params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
        {
            trading::bonk::buy::buy(
                self.rpc.clone(),
                self.payer.clone(),
                mint,
                protocol_params.virtual_base.unwrap_or(0),
                protocol_params.virtual_quote.unwrap_or(0),
                protocol_params.real_base_before.unwrap_or(0),
                protocol_params.real_quote_before.unwrap_or(0),
                buy_sol_cost,
                slippage_basis_points,
                priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.auto_handle_wsol,
            )
            .await
        } else {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }
    }

    /// Sell tokens by percentage
    pub async fn sell_by_percent_use_sell_params(
        &self,
        sell_params: SellParams,
        percent: u64,
    ) -> Result<(), anyhow::Error> {
        let mint = sell_params.mint;
        let creator = sell_params.creator;
        let amount_token = sell_params.amount_token;
        let recent_blockhash = sell_params.recent_blockhash;
        if let Some(_) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunSellParams>()
        {
            trading::pumpfun::sell::sell_by_percent(
                self.rpc.clone(),
                self.payer.clone(),
                mint.clone(),
                creator,
                percent,
                amount_token.unwrap_or(0),
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
        {
            trading::pumpswap::sell::sell_by_percent(
                self.rpc.clone(),
                self.payer.clone(),
                mint.clone(),
                creator,
                percent,
                None,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.pool.clone(),
                protocol_params.pool_base_token_account.clone(),
                protocol_params.pool_quote_token_account.clone(),
                protocol_params.user_base_token_account.clone(),
                protocol_params.user_quote_token_account.clone(),
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
        {
            trading::bonk::sell::sell_by_percent(
                self.rpc.clone(),
                self.payer.clone(),
                mint.clone(),
                protocol_params.virtual_base.unwrap_or(0),
                protocol_params.virtual_quote.unwrap_or(0),
                protocol_params.real_base_before.unwrap_or(0),
                protocol_params.real_quote_before.unwrap_or(0),
                percent,
                None,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else {
            return Err(anyhow::anyhow!("Invalid protocol params for Trade"));
        }
    }

    /// Sell tokens by amount
    pub async fn sell_by_amount_use_sell_params(
        &self,
        sell_params: SellParams,
    ) -> Result<(), anyhow::Error> {
        let mint = sell_params.mint;
        let creator = sell_params.creator;
        let amount = sell_params.amount_token;
        let recent_blockhash = sell_params.recent_blockhash;
        if let Some(_) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunSellParams>()
        {
            trading::pumpfun::sell::sell_by_amount(
                self.rpc.clone(),
                self.payer.clone(),
                mint.clone(),
                creator,
                amount.unwrap_or(0),
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
        {
            trading::pumpswap::sell::sell_by_amount(
                self.rpc.clone(),
                self.payer.clone(),
                mint.clone(),
                creator,
                amount.unwrap_or(0),
                None,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.pool.clone(),
                protocol_params.pool_base_token_account.clone(),
                protocol_params.pool_quote_token_account.clone(),
                protocol_params.user_base_token_account.clone(),
                protocol_params.user_quote_token_account.clone(),
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
        {
            trading::bonk::sell::sell_by_amount(
                self.rpc.clone(),
                self.payer.clone(),
                mint.clone(),
                protocol_params.virtual_base.unwrap_or(0),
                protocol_params.virtual_quote.unwrap_or(0),
                protocol_params.real_base_before.unwrap_or(0),
                protocol_params.real_quote_before.unwrap_or(0),
                amount.unwrap_or(0),
                None,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else {
            Err(anyhow::anyhow!("Invalid protocol params for Trade"))
        }
    }

    pub async fn sell_by_percent_with_tip_use_sell_params(
        &self,
        sell_params: SellWithTipParams,
        percent: u64,
    ) -> Result<(), anyhow::Error> {
        let mint = sell_params.mint;
        let creator = sell_params.creator;
        let amount_token = sell_params.amount_token;
        let recent_blockhash = sell_params.recent_blockhash;
        if let Some(_) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunSellParams>()
        {
            trading::pumpfun::sell::sell_by_percent_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                creator,
                percent,
                amount_token.unwrap_or(0),
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
        {
            trading::pumpswap::sell::sell_by_percent_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                creator,
                percent,
                sell_params.slippage_basis_points,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.pool.clone(),
                protocol_params.pool_base_token_account.clone(),
                protocol_params.pool_quote_token_account.clone(),
                protocol_params.user_base_token_account.clone(),
                protocol_params.user_quote_token_account.clone(),
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
        {
            trading::bonk::sell::sell_by_percent_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                protocol_params.virtual_base.unwrap_or(0),
                protocol_params.virtual_quote.unwrap_or(0),
                protocol_params.real_base_before.unwrap_or(0),
                protocol_params.real_quote_before.unwrap_or(0),
                percent,
                sell_params.slippage_basis_points,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else {
            Err(anyhow::anyhow!("Invalid protocol params for Trade"))
        }
    }

    pub async fn sell_by_amount_with_tip_use_sell_params(
        &self,
        sell_params: SellWithTipParams,
    ) -> Result<(), anyhow::Error> {
        let mint = sell_params.mint;
        let creator = sell_params.creator;
        let amount = sell_params.amount_token;
        let recent_blockhash = sell_params.recent_blockhash;
        if let Some(_) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpFunSellParams>()
        {
            trading::pumpfun::sell::sell_by_amount_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                creator,
                amount.unwrap_or(0),
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<PumpSwapParams>()
        {
            trading::pumpswap::sell::sell_by_amount_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                creator,
                amount.unwrap_or(0),
                sell_params.slippage_basis_points,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
                protocol_params.pool.clone(),
                protocol_params.pool_base_token_account.clone(),
                protocol_params.pool_quote_token_account.clone(),
                protocol_params.user_base_token_account.clone(),
                protocol_params.user_quote_token_account.clone(),
            )
            .await
        } else if let Some(protocol_params) = sell_params
            .protocol_params
            .as_any()
            .downcast_ref::<BonkParams>()
        {
            trading::bonk::sell::sell_by_amount_with_tip(
                self.rpc.clone(),
                self.swqos_clients.clone(),
                self.payer.clone(),
                mint,
                protocol_params.virtual_base.unwrap_or(0),
                protocol_params.virtual_quote.unwrap_or(0),
                protocol_params.real_base_before.unwrap_or(0),
                protocol_params.real_quote_before.unwrap_or(0),
                amount.unwrap_or(0),
                sell_params.slippage_basis_points,
                self.priority_fee.clone(),
                self.trade_config.lookup_table_key,
                recent_blockhash,
            )
            .await
        } else {
            Err(anyhow::anyhow!("Invalid protocol params for Trade"))
        }
    }

    #[inline]
    pub async fn get_sol_balance(&self, payer: &Pubkey) -> Result<u64, anyhow::Error> {
        trading::pumpfun::common::get_sol_balance(&self.rpc, payer).await
    }

    #[inline]
    pub async fn get_payer_sol_balance(&self) -> Result<u64, anyhow::Error> {
        trading::pumpfun::common::get_sol_balance(&self.rpc, &self.payer.pubkey()).await
    }

    #[inline]
    pub async fn get_token_balance(
        &self,
        payer: &Pubkey,
        mint: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        println!(
            "get_token_balance payer: {}, mint: {}, rpc_url: {}",
            payer, mint, self.trade_config.rpc_url
        );
        trading::pumpfun::common::get_token_balance(&self.rpc, payer, mint).await
    }

    #[inline]
    pub async fn get_payer_token_balance(&self, mint: &Pubkey) -> Result<u64, anyhow::Error> {
        trading::pumpfun::common::get_token_balance(&self.rpc, &self.payer.pubkey(), mint).await
    }

    #[inline]
    pub fn get_payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }

    #[inline]
    pub fn get_payer(&self) -> &Keypair {
        self.payer.as_ref()
    }

    #[inline]
    pub fn get_token_price(&self, virtual_sol_reserves: u64, virtual_token_reserves: u64) -> f64 {
        trading::pumpfun::common::get_token_price(virtual_sol_reserves, virtual_token_reserves)
    }

    #[inline]
    pub fn get_buy_price(&self, amount: u64, trade_info: &PumpFunTradeEvent) -> u64 {
        trading::pumpfun::common::get_buy_price(amount, trade_info)
    }

    #[inline]
    pub async fn transfer_sol(
        &self,
        payer: &Keypair,
        receive_wallet: &Pubkey,
        amount: u64,
    ) -> Result<(), anyhow::Error> {
        trading::pumpfun::common::transfer_sol(&self.rpc, payer, receive_wallet, amount).await
    }

    #[inline]
    pub async fn close_token_account(&self, mint: &Pubkey) -> Result<(), anyhow::Error> {
        trading::pumpfun::common::close_token_account(&self.rpc, self.payer.as_ref(), mint).await
    }

    #[inline]
    pub async fn get_current_price(&self, mint: &Pubkey) -> Result<f64, anyhow::Error> {
        let (bonding_curve, _) =
            trading::pumpfun::common::get_bonding_curve_account_v2(&self.rpc, mint).await?;

        let virtual_sol_reserves = bonding_curve.virtual_sol_reserves;
        let virtual_token_reserves = bonding_curve.virtual_token_reserves;

        Ok(trading::pumpfun::common::get_token_price(
            virtual_sol_reserves,
            virtual_token_reserves,
        ))
    }

    #[inline]
    pub async fn get_real_sol_reserves(&self, mint: &Pubkey) -> Result<u64, anyhow::Error> {
        let (bonding_curve, _) =
            trading::pumpfun::common::get_bonding_curve_account_v2(&self.rpc, mint).await?;

        let actual_sol_reserves = bonding_curve.real_sol_reserves;

        Ok(actual_sol_reserves)
    }

    #[inline]
    pub async fn get_creator(&self, mint: &Pubkey) -> Result<Pubkey, anyhow::Error> {
        let (bonding_curve, _) =
            trading::pumpfun::common::get_bonding_curve_account_v2(&self.rpc, mint).await?;

        let creator = bonding_curve.creator;

        Ok(creator)
    }

    #[inline]
    pub async fn get_current_price_with_pumpswap(
        &self,
        pool_address: &Pubkey,
    ) -> Result<f64, anyhow::Error> {
        let pool = trading::pumpswap::pool::Pool::fetch(&self.rpc, pool_address).await?;

        let (base_amount, quote_amount) = pool.get_token_balances(&self.rpc).await?;

        // Calculate price using constant product formula (x * y = k)
        // Price = quote_amount / base_amount
        if base_amount == 0 {
            return Err(anyhow::anyhow!(
                "Base amount is zero, cannot calculate price"
            ));
        }

        let price = quote_amount as f64 / base_amount as f64;

        Ok(price)
    }

    #[inline]
    pub async fn get_real_sol_reserves_with_pumpswap(
        &self,
        pool_address: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        let pool = trading::pumpswap::pool::Pool::fetch(&self.rpc, pool_address).await?;

        let (_, quote_amount) = pool.get_token_balances(&self.rpc).await?;

        Ok(quote_amount)
    }

    #[inline]
    pub async fn get_payer_token_balance_with_pumpswap(
        &self,
        pool_address: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        let pool = trading::pumpswap::pool::Pool::fetch(&self.rpc, pool_address).await?;

        let (base_amount, _) = pool.get_token_balances(&self.rpc).await?;

        Ok(base_amount)
    }
}
