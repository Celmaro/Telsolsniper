use crate::common::spl_associated_token_account::get_associated_token_address_with_program_id;
use crate::common::SolanaRpcClient;
use crate::instruction::utils::pumpswap::accounts::MAYHEM_FEE_RECIPIENT as MAYHEM_FEE_RECIPIENT_SWAP;
use solana_sdk::pubkey::Pubkey;

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
    /// Pool base token account
    pub pool_base_token_account: Pubkey,
    /// Pool quote token account
    pub pool_quote_token_account: Pubkey,
    /// Base token reserves in the pool
    pub pool_base_token_reserves: u64,
    /// Quote token reserves in the pool
    pub pool_quote_token_reserves: u64,
    /// Coin creator vault ATA
    pub coin_creator_vault_ata: Pubkey,
    /// Coin creator vault authority
    pub coin_creator_vault_authority: Pubkey,
    /// Token program ID
    pub base_token_program: Pubkey,
    /// Quote token program ID
    pub quote_token_program: Pubkey,
    /// Whether the pool is in mayhem mode
    pub is_mayhem_mode: bool,
    /// Pool [`Pool::coin_creator`](crate::instruction::utils::pumpswap_types::Pool). Used for PumpSwap
    /// `remaining_accounts`: **`pool-v2` is appended only when this is not `Pubkey::default()`
    /// (matches `@pump-fun/pump-swap-sdk`); wrong flag causes buys to revert with buyback recipient errors (e.g. 6053).
    pub coin_creator: Pubkey,
    /// Whether the pool's coin has cashback enabled
    pub is_cashback_coin: bool,
    /// Cashback fee in basis points (from trade events / sol-parser-sdk). For quote-in buy and base-in sell
    /// math, this is summed with [`COIN_CREATOR_FEE_BASIS_POINTS`](crate::instruction::utils::pumpswap::accounts::COIN_CREATOR_FEE_BASIS_POINTS)
    /// when a creator vault applies — matching on-chain treating creator + cashback as one fee bucket.
    /// Use `0` when unknown (e.g. RPC-only pool decode has no per-mint cashback bps).
    pub cashback_fee_basis_points: u64,
}

impl PumpSwapParams {
    pub fn new(
        pool: Pubkey,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        pool_base_token_account: Pubkey,
        pool_quote_token_account: Pubkey,
        pool_base_token_reserves: u64,
        pool_quote_token_reserves: u64,
        coin_creator_vault_ata: Pubkey,
        coin_creator_vault_authority: Pubkey,
        base_token_program: Pubkey,
        quote_token_program: Pubkey,
        fee_recipient: Pubkey,
        coin_creator: Pubkey,
        is_cashback_coin: bool,
        cashback_fee_basis_points: u64,
    ) -> Self {
        let is_mayhem_mode = fee_recipient == MAYHEM_FEE_RECIPIENT_SWAP;
        Self {
            pool,
            base_mint,
            quote_mint,
            pool_base_token_account,
            pool_quote_token_account,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            coin_creator_vault_ata,
            coin_creator_vault_authority,
            base_token_program,
            quote_token_program,
            is_mayhem_mode,
            coin_creator,
            is_cashback_coin,
            cashback_fee_basis_points,
        }
    }

    /// Fast-path constructor for building PumpSwap parameters directly from decoded
    /// trade/event data and the accompanying instruction accounts, avoiding RPC
    /// lookups and associated latency. Token program IDs should be sourced from
    /// the instruction accounts themselves to respect Token Program vs Token-2022
    /// differences.
    ///
    /// When building from event/parser (e.g. sol-parser-sdk), pass `is_cashback_coin`
    /// from the event so that buy/sell instructions include the correct remaining
    /// accounts for cashback.
    pub fn from_trade(
        pool: Pubkey,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        pool_base_token_account: Pubkey,
        pool_quote_token_account: Pubkey,
        pool_base_token_reserves: u64,
        pool_quote_token_reserves: u64,
        coin_creator_vault_ata: Pubkey,
        coin_creator_vault_authority: Pubkey,
        base_token_program: Pubkey,
        quote_token_program: Pubkey,
        fee_recipient: Pubkey,
        coin_creator: Pubkey,
        is_cashback_coin: bool,
        cashback_fee_basis_points: u64,
    ) -> Self {
        Self::new(
            pool,
            base_mint,
            quote_mint,
            pool_base_token_account,
            pool_quote_token_account,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            coin_creator_vault_ata,
            coin_creator_vault_authority,
            base_token_program,
            quote_token_program,
            fee_recipient,
            coin_creator,
            is_cashback_coin,
            cashback_fee_basis_points,
        )
    }

    pub async fn from_mint_by_rpc(
        rpc: &SolanaRpcClient,
        mint: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        if let Ok((pool_address, _)) =
            crate::instruction::utils::pumpswap::find_by_base_mint(rpc, mint).await
        {
            Self::from_pool_address_by_rpc(rpc, &pool_address).await
        } else if let Ok((pool_address, _)) =
            crate::instruction::utils::pumpswap::find_by_quote_mint(rpc, mint).await
        {
            Self::from_pool_address_by_rpc(rpc, &pool_address).await
        } else {
            return Err(anyhow::anyhow!("No pool found for mint"));
        }
    }

    pub async fn from_pool_address_by_rpc(
        rpc: &SolanaRpcClient,
        pool_address: &Pubkey,
    ) -> Result<Self, anyhow::Error> {
        let pool_data = crate::instruction::utils::pumpswap::fetch_pool(rpc, pool_address).await?;
        Self::from_pool_data(rpc, pool_address, &pool_data).await
    }

    /// Build params from an already-decoded Pool, only fetching token balances.
    ///
    /// Saves 1 RPC `getAccount` call vs `from_pool_address_by_rpc` when pool data
    /// is already available (e.g. from `pumpswap::find_by_mint` which returns the
    /// decoded Pool).
    pub async fn from_pool_data(
        rpc: &SolanaRpcClient,
        pool_address: &Pubkey,
        pool_data: &crate::instruction::utils::pumpswap_types::Pool,
    ) -> Result<Self, anyhow::Error> {
        let (pool_base_token_reserves, pool_quote_token_reserves) =
            crate::instruction::utils::pumpswap::get_token_balances(pool_data, rpc).await?;
        let creator = pool_data.coin_creator;
        let coin_creator_vault_ata = crate::instruction::utils::pumpswap::coin_creator_vault_ata(
            creator,
            pool_data.quote_mint,
        );
        let coin_creator_vault_authority =
            crate::instruction::utils::pumpswap::coin_creator_vault_authority(creator);

        let base_token_program_ata = get_associated_token_address_with_program_id(
            pool_address,
            &pool_data.base_mint,
            &crate::constants::TOKEN_PROGRAM,
        );
        let quote_token_program_ata = get_associated_token_address_with_program_id(
            pool_address,
            &pool_data.quote_mint,
            &crate::constants::TOKEN_PROGRAM,
        );

        Ok(Self {
            pool: *pool_address,
            base_mint: pool_data.base_mint,
            quote_mint: pool_data.quote_mint,
            pool_base_token_account: pool_data.pool_base_token_account,
            pool_quote_token_account: pool_data.pool_quote_token_account,
            pool_base_token_reserves,
            pool_quote_token_reserves,
            coin_creator_vault_ata,
            coin_creator_vault_authority,
            base_token_program: if pool_data.pool_base_token_account == base_token_program_ata {
                crate::constants::TOKEN_PROGRAM
            } else {
                crate::constants::TOKEN_PROGRAM_2022
            },
            is_cashback_coin: pool_data.is_cashback_coin,
            quote_token_program: if pool_data.pool_quote_token_account == quote_token_program_ata {
                crate::constants::TOKEN_PROGRAM
            } else {
                crate::constants::TOKEN_PROGRAM_2022
            },
            is_mayhem_mode: pool_data.is_mayhem_mode,
            coin_creator: pool_data.coin_creator,
            cashback_fee_basis_points: 0,
        })
    }
}
