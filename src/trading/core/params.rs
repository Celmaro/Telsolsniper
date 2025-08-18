use solana_hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use solana_streamer_sdk::streaming::event_parser::protocols::pumpswap::{
    PumpSwapBuyEvent, PumpSwapSellEvent,
};
use std::sync::Arc;

use super::traits::ProtocolParams;
use crate::common::bonding_curve::BondingCurveAccount;
use crate::common::{PriorityFee, SolanaRpcClient};
use crate::constants::bonk::accounts::{
    self, PLATFORM_FEE_RATE, PROTOCOL_FEE_RATE, SHARE_FEE_RATE,
};
use crate::solana_streamer_sdk::streaming::event_parser::common::EventType;
use crate::solana_streamer_sdk::streaming::event_parser::protocols::bonk::BonkTradeEvent;
use crate::swqos::SwqosClient;
use crate::trading::bonk::common::{get_amount_in, get_amount_in_net, get_amount_out};
use crate::trading::pumpswap::common::get_token_balances;
use crate::trading::raydium_cpmm::common::get_pool_token_balances;

/// Common buy parameters
/// Contains all necessary information for executing buy transactions
#[derive(Clone)]
pub struct BuyParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub sol_amount: u64,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub data_size_limit: u32,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// Buy parameters with MEV service support
/// Extends BuyParams with MEV client configurations for transaction acceleration
#[derive(Clone)]
pub struct BuyWithTipParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub swqos_clients: Vec<Arc<SwqosClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub sol_amount: u64,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub data_size_limit: u32,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// Common sell parameters
/// Contains all necessary information for executing sell transactions
#[derive(Clone)]
pub struct SellParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub token_amount: Option<u64>,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// Sell parameters with MEV service support
/// Extends SellParams with MEV client configurations for transaction acceleration
#[derive(Clone)]
pub struct SellWithTipParams {
    pub rpc: Option<Arc<SolanaRpcClient>>,
    pub swqos_clients: Vec<Arc<SwqosClient>>,
    pub payer: Arc<Keypair>,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub token_amount: Option<u64>,
    pub slippage_basis_points: Option<u64>,
    pub priority_fee: PriorityFee,
    pub lookup_table_key: Option<Pubkey>,
    pub recent_blockhash: Hash,
    pub protocol_params: Box<dyn ProtocolParams>,
}

/// PumpFun protocol specific parameters
/// Configuration parameters specific to PumpFun trading protocol
#[derive(Clone)]
pub struct PumpFunParams {
    pub bonding_curve: Arc<BondingCurveAccount>,
    /// Whether to close token account when selling, only effective during sell operations
    pub close_token_account_when_sell: Option<bool>,
}

impl PumpFunParams {
    pub fn from_dev_trade(
        mint: &Pubkey,
        dev_token_amount: u64,
        dev_sol_amount: u64,
        creator: Pubkey,
        close_token_account_when_sell: Option<bool>,
    ) -> Self {
        let bonding_curve =
            BondingCurveAccount::from_dev_trade(mint, dev_token_amount, dev_sol_amount, creator);
        Self {
            bonding_curve: Arc::new(bonding_curve),
            close_token_account_when_sell: close_token_account_when_sell,
        }
    }

    pub fn from_trade(
        event: &PumpFunTradeEvent,
        close_token_account_when_sell: Option<bool>,
    ) -> Self {
        let bonding_curve = BondingCurveAccount::from_trade(event);
        Self {
            bonding_curve: Arc::new(bonding_curve),
            close_token_account_when_sell: close_token_account_when_sell,
        }
    }
}

impl ProtocolParams for PumpFunParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

/// PumpSwap Protocol Specific Parameters
///
/// Parameters for configuring PumpSwap trading protocol, including liquidity pool information,
/// token configuration, and transaction amounts.
///
/// **Performance Note**: If these parameters are not provided, the system will attempt to
/// retrieve the relevant information from RPC, which will increase transaction time.
/// For optimal performance, it is recommended to provide all necessary parameters in advance.
#[derive(Clone)]
pub struct PumpSwapParams {
    /// Liquidity pool address
    pub pool: Pubkey,
    /// Base token mint address
    /// The mint account address of the base token in the trading pair
    pub base_mint: Pubkey,
    /// Quote token mint address
    /// The mint account address of the quote token in the trading pair, usually SOL or USDC
    pub quote_mint: Pubkey,
    /// Base token reserves in the pool
    pub pool_base_token_reserves: u64,
    /// Quote token reserves in the pool
    pub pool_quote_token_reserves: u64,
    /// Automatically handle WSOL wrapping
    /// When true, automatically handles wrapping and unwrapping operations between SOL and WSOL
    pub auto_handle_wsol: bool,
}

impl PumpSwapParams {
    pub fn from_buy_trade(event: &PumpSwapBuyEvent) -> Self {
        Self {
            pool: event.pool,
            base_mint: event.base_mint,
            quote_mint: event.quote_mint,
            pool_base_token_reserves: event.pool_base_token_reserves,
            pool_quote_token_reserves: event.pool_quote_token_reserves,
            auto_handle_wsol: true,
        }
    }

    pub fn from_sell_trade(event: &PumpSwapSellEvent) -> Self {
        Self {
            pool: event.pool,
            base_mint: event.base_mint,
            quote_mint: event.quote_mint,
            pool_base_token_reserves: event.pool_base_token_reserves,
            pool_quote_token_reserves: event.pool_quote_token_reserves,
            auto_handle_wsol: true,
        }
    }

    pub async fn from_pool_address_by_rpc(
        rpc: &SolanaRpcClient,
        pool_address: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let pool_data = crate::trading::pumpswap::common::fetch_pool(rpc, pool_address).await?;
        let (pool_base_token_reserves, pool_quote_token_reserves) =
            get_token_balances(&pool_data, rpc).await?;
        Ok(Self {
            pool: pool_address.clone(),
            base_mint: pool_data.base_mint,
            quote_mint: pool_data.quote_mint,
            pool_base_token_reserves: pool_base_token_reserves,
            pool_quote_token_reserves: pool_quote_token_reserves,
            auto_handle_wsol: true,
        })
    }
}

impl ProtocolParams for PumpSwapParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

/// Bonk protocol specific parameters
/// Configuration parameters specific to Bonk trading protocol
#[derive(Clone)]
pub struct BonkParams {
    pub virtual_base: u128,
    pub virtual_quote: u128,
    pub real_base: u128,
    pub real_quote: u128,
    /// Token program ID
    /// Specifies the program used by the token, usually spl_token::ID or spl_token_2022::ID
    pub mint_token_program: Pubkey,
    pub auto_handle_wsol: bool,
}

impl BonkParams {
    pub fn from_trade(trade_info: BonkTradeEvent) -> Self {
        Self {
            virtual_base: trade_info.virtual_base as u128,
            virtual_quote: trade_info.virtual_quote as u128,
            real_base: trade_info.real_base_after as u128,
            real_quote: trade_info.real_quote_after as u128,
            mint_token_program: trade_info.base_token_program,
            auto_handle_wsol: true,
        }
    }

    pub fn from_dev_trade(trade_info: BonkTradeEvent) -> Self {
        const DEFAULT_VIRTUAL_BASE: u128 = 1073025605596382;
        const DEFAULT_VIRTUAL_QUOTE: u128 = 30000852951;
        let amount_in = if trade_info.metadata.event_type == EventType::BonkBuyExactIn {
            trade_info.amount_in
        } else {
            get_amount_in(
                trade_info.amount_out,
                PROTOCOL_FEE_RATE,
                PLATFORM_FEE_RATE,
                SHARE_FEE_RATE,
                DEFAULT_VIRTUAL_BASE,
                DEFAULT_VIRTUAL_QUOTE,
                0,
                0,
                0,
            )
        };
        let real_quote =
            get_amount_in_net(amount_in, PROTOCOL_FEE_RATE, PLATFORM_FEE_RATE, SHARE_FEE_RATE)
                as u128;
        let amount_out = if trade_info.metadata.event_type == EventType::BonkBuyExactIn {
            get_amount_out(
                trade_info.amount_in,
                PROTOCOL_FEE_RATE,
                PLATFORM_FEE_RATE,
                SHARE_FEE_RATE,
                DEFAULT_VIRTUAL_BASE,
                DEFAULT_VIRTUAL_QUOTE,
                0,
                0,
                0,
            ) as u128
        } else {
            trade_info.amount_out as u128
        };
        let real_base = amount_out;
        Self {
            virtual_base: DEFAULT_VIRTUAL_BASE,
            virtual_quote: DEFAULT_VIRTUAL_QUOTE,
            real_base: real_base,
            real_quote: real_quote,
            mint_token_program: trade_info.base_token_program,
            auto_handle_wsol: true,
        }
    }

    pub async fn from_mint_by_rpc(
        rpc: &SolanaRpcClient,
        mint: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let pool_address =
            crate::trading::bonk::common::get_pool_pda(mint, &accounts::WSOL_TOKEN_ACCOUNT)
                .unwrap();
        let pool_data = crate::trading::bonk::common::fetch_pool_state(rpc, &pool_address).await?;
        let token_account = rpc.get_account(&pool_data.base_mint).await?;
        Ok(Self {
            virtual_base: pool_data.virtual_base as u128,
            virtual_quote: pool_data.virtual_quote as u128,
            real_base: pool_data.real_base as u128,
            real_quote: pool_data.real_quote as u128,
            mint_token_program: token_account.owner,
            auto_handle_wsol: true,
        })
    }
}

impl ProtocolParams for BonkParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

/// RaydiumCpmm protocol specific parameters
/// Configuration parameters specific to Raydium CPMM trading protocol
#[derive(Clone)]
pub struct RaydiumCpmmParams {
    /// Base token mint address
    pub base_mint: Pubkey,
    /// Quote token mint address
    pub quote_mint: Pubkey,
    /// Base token reserve amount in the pool
    pub base_reserve: u64,
    /// Quote token reserve amount in the pool
    pub quote_reserve: u64,
    /// Base token program ID (usually spl_token::ID or spl_token_2022::ID)
    pub base_token_program: Pubkey,
    /// Quote token program ID (usually spl_token::ID or spl_token_2022::ID)
    pub quote_token_program: Pubkey,
    /// Whether to automatically handle wSOL wrapping and unwrapping
    pub auto_handle_wsol: bool,
}

impl RaydiumCpmmParams {
    pub async fn from_pool_address_by_rpc(
        rpc: &SolanaRpcClient,
        pool_address: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let pool =
            crate::trading::raydium_cpmm::common::fetch_pool_state(rpc, pool_address).await?;
        let (token0_balance, token1_balance) =
            get_pool_token_balances(rpc, pool_address, &pool.token0_mint, &pool.token1_mint)
                .await?;
        Ok(Self {
            base_mint: pool.token0_mint,
            quote_mint: pool.token1_mint,
            base_reserve: token0_balance,
            quote_reserve: token1_balance,
            base_token_program: pool.token0_program,
            quote_token_program: pool.token1_program,
            auto_handle_wsol: true,
        })
    }
}

impl ProtocolParams for RaydiumCpmmParams {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProtocolParams> {
        Box::new(self.clone())
    }
}

impl BuyParams {
    /// Convert to BuyWithTipParams
    /// Transforms basic buy parameters into MEV-enabled parameters
    pub fn with_tip(self, swqos_clients: Vec<Arc<SwqosClient>>) -> BuyWithTipParams {
        BuyWithTipParams {
            rpc: self.rpc,
            swqos_clients,
            payer: self.payer,
            mint: self.mint,
            creator: self.creator,
            sol_amount: self.sol_amount,
            slippage_basis_points: self.slippage_basis_points,
            priority_fee: self.priority_fee,
            lookup_table_key: self.lookup_table_key,
            recent_blockhash: self.recent_blockhash,
            data_size_limit: self.data_size_limit,
            protocol_params: self.protocol_params,
        }
    }
}

impl SellParams {
    /// Convert to SellWithTipParams
    /// Transforms basic sell parameters into MEV-enabled parameters
    pub fn with_tip(self, swqos_clients: Vec<Arc<SwqosClient>>) -> SellWithTipParams {
        SellWithTipParams {
            rpc: self.rpc,
            swqos_clients,
            payer: self.payer,
            mint: self.mint,
            creator: self.creator,
            token_amount: self.token_amount,
            slippage_basis_points: self.slippage_basis_points,
            priority_fee: self.priority_fee,
            lookup_table_key: self.lookup_table_key,
            recent_blockhash: self.recent_blockhash,
            protocol_params: self.protocol_params,
        }
    }
}
