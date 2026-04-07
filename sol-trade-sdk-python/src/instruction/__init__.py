"""
Solana Trading SDK - Instruction Builders

Production-grade instruction builders for various Solana DEX protocols:
- PumpFun
- PumpSwap
- Bonk
- Raydium CPMM
- Raydium AMM V4
- Meteora DAMM V2

Each module provides:
- Program IDs and constants
- Instruction discriminators
- PDA derivation functions
- build_buy_instructions function
- build_sell_instructions function
- Protocol-specific parameter dataclasses
"""

from .common import (
    # Program IDs
    SYSTEM_PROGRAM,
    TOKEN_PROGRAM,
    TOKEN_PROGRAM_2022,
    ASSOCIATED_TOKEN_PROGRAM,
    # Token Mints
    SOL_TOKEN_ACCOUNT,
    WSOL_TOKEN_ACCOUNT,
    USDC_TOKEN_ACCOUNT,
    USD1_TOKEN_ACCOUNT,
    # Constants
    DEFAULT_SLIPPAGE,
    # Utility Functions
    get_associated_token_address,
    create_associated_token_account_idempotent_instruction,
    create_wsol_account_instruction,
    close_wsol_account_instruction,
    close_token_account_instruction,
    handle_wsol,
    create_wsol_ata,
    close_wsol,
    # Calculation Functions
    calculate_with_slippage_buy,
    calculate_with_slippage_sell,
)

from .pumpfun_builder import (
    # Program IDs and Constants
    PUMPFUN_PROGRAM_ID,
    FEE_RECIPIENT as PUMPFUN_FEE_RECIPIENT,
    GLOBAL_ACCOUNT as PUMPFUN_GLOBAL_ACCOUNT,
    EVENT_AUTHORITY as PUMPFUN_EVENT_AUTHORITY,
    AUTHORITY as PUMPFUN_AUTHORITY,
    FEE_PROGRAM as PUMPFUN_FEE_PROGRAM,
    GLOBAL_VOLUME_ACCUMULATOR as PUMPFUN_GLOBAL_VOLUME_ACCUMULATOR,
    FEE_CONFIG as PUMPFUN_FEE_CONFIG,
    MAYHEM_FEE_RECIPIENTS as PUMPFUN_MAYHEM_FEE_RECIPIENTS,
    # Discriminators
    BUY_DISCRIMINATOR as PUMPFUN_BUY_DISCRIMINATOR,
    BUY_EXACT_SOL_IN_DISCRIMINATOR as PUMPFUN_BUY_EXACT_SOL_IN_DISCRIMINATOR,
    SELL_DISCRIMINATOR as PUMPFUN_SELL_DISCRIMINATOR,
    CLAIM_CASHBACK_DISCRIMINATOR as PUMPFUN_CLAIM_CASHBACK_DISCRIMINATOR,
    # PDA Functions
    get_bonding_curve_pda,
    get_bonding_curve_v2_pda,
    get_creator_vault_pda,
    get_user_volume_accumulator_pda as get_pumpfun_user_volume_accumulator_pda,
    get_creator,
    get_mayhem_fee_recipient_random,
    # Params
    PumpFunParams,
    # Calculation Functions
    get_buy_token_amount_from_sol_amount,
    get_sell_sol_amount_from_token_amount,
    # Instruction Builders
    build_buy_instructions as build_pumpfun_buy_instructions,
    build_sell_instructions as build_pumpfun_sell_instructions,
    claim_cashback_pumpfun_instruction,
)

from .pumpswap_builder import (
    # Program IDs and Constants
    PUMPSWAP_PROGRAM_ID,
    FEE_RECIPIENT as PUMPSWAP_FEE_RECIPIENT,
    GLOBAL_ACCOUNT as PUMPSWAP_GLOBAL_ACCOUNT,
    EVENT_AUTHORITY as PUMPSWAP_EVENT_AUTHORITY,
    ASSOCIATED_TOKEN_PROGRAM as PUMPSWAP_ASSOCIATED_TOKEN_PROGRAM,
    PUMP_PROGRAM_ID,
    FEE_PROGRAM as PUMPSWAP_FEE_PROGRAM,
    GLOBAL_VOLUME_ACCUMULATOR as PUMPSWAP_GLOBAL_VOLUME_ACCUMULATOR,
    FEE_CONFIG as PUMPSWAP_FEE_CONFIG,
    DEFAULT_COIN_CREATOR_VAULT_AUTHORITY,
    MAYHEM_FEE_RECIPIENTS as PUMPSWAP_MAYHEM_FEE_RECIPIENTS,
    LP_FEE_BASIS_POINTS,
    PROTOCOL_FEE_BASIS_POINTS,
    COIN_CREATOR_FEE_BASIS_POINTS,
    # Discriminators
    BUY_DISCRIMINATOR as PUMPSWAP_BUY_DISCRIMINATOR,
    BUY_EXACT_QUOTE_IN_DISCRIMINATOR,
    SELL_DISCRIMINATOR as PUMPSWAP_SELL_DISCRIMINATOR,
    CLAIM_CASHBACK_DISCRIMINATOR as PUMPSWAP_CLAIM_CASHBACK_DISCRIMINATOR,
    # PDA Functions
    get_pool_v2_pda,
    get_pump_pool_authority_pda,
    get_canonical_pool_pda,
    get_user_volume_accumulator_pda as get_pumpswap_user_volume_accumulator_pda,
    get_user_volume_accumulator_wsol_ata,
    get_user_volume_accumulator_quote_ata,
    coin_creator_vault_authority,
    coin_creator_vault_ata,
    fee_recipient_ata,
    get_mayhem_fee_recipient_random as get_pumpswap_mayhem_fee_recipient_random,
    # Params
    PumpSwapParams,
    # Calculation Functions
    buy_quote_input_internal,
    sell_base_input_internal,
    # Instruction Builders
    build_buy_instructions as build_pumpswap_buy_instructions,
    build_sell_instructions as build_pumpswap_sell_instructions,
    claim_cashback_pumpswap_instruction,
)

from .bonk_builder import (
    # Program IDs and Constants
    BONK_PROGRAM_ID,
    AUTHORITY as BONK_AUTHORITY,
    GLOBAL_CONFIG,
    USD1_GLOBAL_CONFIG,
    EVENT_AUTHORITY as BONK_EVENT_AUTHORITY,
    PLATFORM_FEE_RATE,
    PROTOCOL_FEE_RATE as BONK_PROTOCOL_FEE_RATE,
    SHARE_FEE_RATE,
    # Discriminators
    BUY_EXACT_IN_DISCRIMINATOR,
    SELL_EXACT_IN_DISCRIMINATOR,
    # PDA Functions
    get_pool_pda as get_bonk_pool_pda,
    get_vault_pda as get_bonk_vault_pda,
    get_platform_associated_account,
    get_creator_associated_account,
    # Params
    BonkParams,
    # Calculation Functions
    get_amount_in_net,
    get_amount_out as get_bonk_amount_out,
    get_buy_token_amount_from_sol_amount as get_bonk_buy_token_amount_from_sol_amount,
    get_sell_sol_amount_from_token_amount as get_bonk_sell_sol_amount_from_token_amount,
    # Instruction Builders
    build_buy_instructions as build_bonk_buy_instructions,
    build_sell_instructions as build_bonk_sell_instructions,
)

from .raydium_cpmm_builder import (
    # Program IDs and Constants
    RAYDIUM_CPMM_PROGRAM_ID,
    AUTHORITY as RAYDIUM_CPMM_AUTHORITY,
    FEE_RATE_DENOMINATOR_VALUE,
    TRADE_FEE_RATE,
    CREATOR_FEE_RATE,
    PROTOCOL_FEE_RATE as RAYDIUM_CPMM_PROTOCOL_FEE_RATE,
    FUND_FEE_RATE,
    # Discriminators
    SWAP_BASE_IN_DISCRIMINATOR as RAYDIUM_CPMM_SWAP_BASE_IN_DISCRIMINATOR,
    SWAP_BASE_OUT_DISCRIMINATOR as RAYDIUM_CPMM_SWAP_BASE_OUT_DISCRIMINATOR,
    # PDA Functions
    get_pool_pda as get_raydium_cpmm_pool_pda,
    get_vault_pda as get_raydium_cpmm_vault_pda,
    get_observation_state_pda,
    # Params
    RaydiumCpmmParams,
    # Calculation Functions
    compute_swap_amount as compute_raydium_cpmm_swap_amount,
    # Instruction Builders
    build_buy_instructions as build_raydium_cpmm_buy_instructions,
    build_sell_instructions as build_raydium_cpmm_sell_instructions,
)

from .raydium_amm_v4_builder import (
    # Program IDs and Constants
    RAYDIUM_AMM_V4_PROGRAM_ID,
    AUTHORITY as RAYDIUM_AMM_V4_AUTHORITY,
    TRADE_FEE_NUMERATOR,
    TRADE_FEE_DENOMINATOR,
    SWAP_FEE_NUMERATOR,
    SWAP_FEE_DENOMINATOR,
    # Discriminators
    SWAP_BASE_IN_DISCRIMINATOR as RAYDIUM_AMM_V4_SWAP_BASE_IN_DISCRIMINATOR,
    SWAP_BASE_OUT_DISCRIMINATOR as RAYDIUM_AMM_V4_SWAP_BASE_OUT_DISCRIMINATOR,
    # Params
    RaydiumAmmV4Params,
    # Calculation Functions
    compute_swap_amount as compute_raydium_amm_v4_swap_amount,
    # Instruction Builders
    build_buy_instructions as build_raydium_amm_v4_buy_instructions,
    build_sell_instructions as build_raydium_amm_v4_sell_instructions,
)

from .meteora_damm_v2_builder import (
    # Program IDs and Constants
    METEORA_DAMM_V2_PROGRAM_ID,
    AUTHORITY as METEORA_DAMM_V2_AUTHORITY,
    # Discriminators
    SWAP_DISCRIMINATOR as METEORA_DAMM_V2_SWAP_DISCRIMINATOR,
    # PDA Functions
    get_event_authority_pda,
    # Params
    MeteoraDammV2Params,
    # Instruction Builders
    build_buy_instructions as build_meteora_damm_v2_buy_instructions,
    build_sell_instructions as build_meteora_damm_v2_sell_instructions,
)

# Version
__version__ = "1.0.0"

__all__ = [
    # Common
    "SYSTEM_PROGRAM",
    "TOKEN_PROGRAM",
    "TOKEN_PROGRAM_2022",
    "ASSOCIATED_TOKEN_PROGRAM",
    "SOL_TOKEN_ACCOUNT",
    "WSOL_TOKEN_ACCOUNT",
    "USDC_TOKEN_ACCOUNT",
    "USD1_TOKEN_ACCOUNT",
    "DEFAULT_SLIPPAGE",
    "get_associated_token_address",
    "create_associated_token_account_idempotent_instruction",
    "create_wsol_account_instruction",
    "close_wsol_account_instruction",
    "close_token_account_instruction",
    "handle_wsol",
    "create_wsol_ata",
    "close_wsol",
    "calculate_with_slippage_buy",
    "calculate_with_slippage_sell",

    # PumpFun
    "PUMPFUN_PROGRAM_ID",
    "PUMPFUN_FEE_RECIPIENT",
    "PUMPFUN_GLOBAL_ACCOUNT",
    "PUMPFUN_EVENT_AUTHORITY",
    "PUMPFUN_AUTHORITY",
    "PUMPFUN_FEE_PROGRAM",
    "PUMPFUN_GLOBAL_VOLUME_ACCUMULATOR",
    "PUMPFUN_FEE_CONFIG",
    "PUMPFUN_MAYHEM_FEE_RECIPIENTS",
    "PUMPFUN_BUY_DISCRIMINATOR",
    "PUMPFUN_BUY_EXACT_SOL_IN_DISCRIMINATOR",
    "PUMPFUN_SELL_DISCRIMINATOR",
    "PUMPFUN_CLAIM_CASHBACK_DISCRIMINATOR",
    "get_bonding_curve_pda",
    "get_bonding_curve_v2_pda",
    "get_creator_vault_pda",
    "get_pumpfun_user_volume_accumulator_pda",
    "get_creator",
    "get_mayhem_fee_recipient_random",
    "PumpFunParams",
    "get_buy_token_amount_from_sol_amount",
    "get_sell_sol_amount_from_token_amount",
    "build_pumpfun_buy_instructions",
    "build_pumpfun_sell_instructions",
    "claim_cashback_pumpfun_instruction",

    # PumpSwap
    "PUMPSWAP_PROGRAM_ID",
    "PUMPSWAP_FEE_RECIPIENT",
    "PUMPSWAP_GLOBAL_ACCOUNT",
    "PUMPSWAP_EVENT_AUTHORITY",
    "PUMPSWAP_ASSOCIATED_TOKEN_PROGRAM",
    "PUMP_PROGRAM_ID",
    "PUMPSWAP_FEE_PROGRAM",
    "PUMPSWAP_GLOBAL_VOLUME_ACCUMULATOR",
    "PUMPSWAP_FEE_CONFIG",
    "DEFAULT_COIN_CREATOR_VAULT_AUTHORITY",
    "PUMPSWAP_MAYHEM_FEE_RECIPIENTS",
    "LP_FEE_BASIS_POINTS",
    "PROTOCOL_FEE_BASIS_POINTS",
    "COIN_CREATOR_FEE_BASIS_POINTS",
    "PUMPSWAP_BUY_DISCRIMINATOR",
    "BUY_EXACT_QUOTE_IN_DISCRIMINATOR",
    "PUMPSWAP_SELL_DISCRIMINATOR",
    "PUMPSWAP_CLAIM_CASHBACK_DISCRIMINATOR",
    "get_pool_v2_pda",
    "get_pump_pool_authority_pda",
    "get_canonical_pool_pda",
    "get_pumpswap_user_volume_accumulator_pda",
    "get_user_volume_accumulator_wsol_ata",
    "get_user_volume_accumulator_quote_ata",
    "coin_creator_vault_authority",
    "coin_creator_vault_ata",
    "fee_recipient_ata",
    "get_pumpswap_mayhem_fee_recipient_random",
    "PumpSwapParams",
    "buy_quote_input_internal",
    "sell_base_input_internal",
    "build_pumpswap_buy_instructions",
    "build_pumpswap_sell_instructions",
    "claim_cashback_pumpswap_instruction",

    # Bonk
    "BONK_PROGRAM_ID",
    "BONK_AUTHORITY",
    "GLOBAL_CONFIG",
    "USD1_GLOBAL_CONFIG",
    "BONK_EVENT_AUTHORITY",
    "PLATFORM_FEE_RATE",
    "BONK_PROTOCOL_FEE_RATE",
    "SHARE_FEE_RATE",
    "BUY_EXACT_IN_DISCRIMINATOR",
    "SELL_EXACT_IN_DISCRIMINATOR",
    "get_bonk_pool_pda",
    "get_bonk_vault_pda",
    "get_platform_associated_account",
    "get_creator_associated_account",
    "BonkParams",
    "get_amount_in_net",
    "get_bonk_amount_out",
    "get_bonk_buy_token_amount_from_sol_amount",
    "get_bonk_sell_sol_amount_from_token_amount",
    "build_bonk_buy_instructions",
    "build_bonk_sell_instructions",

    # Raydium CPMM
    "RAYDIUM_CPMM_PROGRAM_ID",
    "RAYDIUM_CPMM_AUTHORITY",
    "FEE_RATE_DENOMINATOR_VALUE",
    "TRADE_FEE_RATE",
    "CREATOR_FEE_RATE",
    "RAYDIUM_CPMM_PROTOCOL_FEE_RATE",
    "FUND_FEE_RATE",
    "RAYDIUM_CPMM_SWAP_BASE_IN_DISCRIMINATOR",
    "RAYDIUM_CPMM_SWAP_BASE_OUT_DISCRIMINATOR",
    "get_raydium_cpmm_pool_pda",
    "get_raydium_cpmm_vault_pda",
    "get_observation_state_pda",
    "RaydiumCpmmParams",
    "compute_raydium_cpmm_swap_amount",
    "build_raydium_cpmm_buy_instructions",
    "build_raydium_cpmm_sell_instructions",

    # Raydium AMM V4
    "RAYDIUM_AMM_V4_PROGRAM_ID",
    "RAYDIUM_AMM_V4_AUTHORITY",
    "TRADE_FEE_NUMERATOR",
    "TRADE_FEE_DENOMINATOR",
    "SWAP_FEE_NUMERATOR",
    "SWAP_FEE_DENOMINATOR",
    "RAYDIUM_AMM_V4_SWAP_BASE_IN_DISCRIMINATOR",
    "RAYDIUM_AMM_V4_SWAP_BASE_OUT_DISCRIMINATOR",
    "RaydiumAmmV4Params",
    "compute_raydium_amm_v4_swap_amount",
    "build_raydium_amm_v4_buy_instructions",
    "build_raydium_amm_v4_sell_instructions",

    # Meteora DAMM V2
    "METEORA_DAMM_V2_PROGRAM_ID",
    "METEORA_DAMM_V2_AUTHORITY",
    "METEORA_DAMM_V2_SWAP_DISCRIMINATOR",
    "get_event_authority_pda",
    "MeteoraDammV2Params",
    "build_meteora_damm_v2_buy_instructions",
    "build_meteora_damm_v2_sell_instructions",
]
