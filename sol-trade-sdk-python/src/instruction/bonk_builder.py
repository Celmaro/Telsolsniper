"""
Bonk instruction builder for Solana trading SDK.
Production-grade implementation with all constants, discriminators, and PDA derivation functions.
"""

from typing import List, Optional, Tuple
from dataclasses import dataclass
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import struct

from .common import (
    SYSTEM_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_TOKEN_ACCOUNT,
    USD1_TOKEN_ACCOUNT,
    USDC_TOKEN_ACCOUNT,
    DEFAULT_SLIPPAGE,
    get_associated_token_address,
    create_associated_token_account_idempotent_instruction,
    handle_wsol,
    close_wsol,
    close_token_account_instruction,
    calculate_with_slippage_sell,
)

# ============================================
# Bonk Program ID
# ============================================

BONK_PROGRAM_ID: Pubkey = Pubkey.from_string("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj")

# ============================================
# Bonk Constants
# ============================================

# Authority
AUTHORITY: Pubkey = Pubkey.from_string("WLHv2UAZm6z4KyaaELi5pjdbJh6RESMva1Rnn8pJVVh")

# Global Config (SOL pools)
GLOBAL_CONFIG: Pubkey = Pubkey.from_string("6s1xP3hpbAfFoNtUNF8mfHsjr2Bd97JxFJRWLbL6aHuX")

# USD1 Global Config
USD1_GLOBAL_CONFIG: Pubkey = Pubkey.from_string("EPiZbnrThjyLnoQ6QQzkxeFqyL5uyg9RzNHHAudUPxBz")

# Event Authority
EVENT_AUTHORITY: Pubkey = Pubkey.from_string("2DPAtwB8L12vrMRExbLuyGnC7n2J5LNoZQSejeQGpwkr")

# Fee Rates
PLATFORM_FEE_RATE: int = 100  # 1%
PROTOCOL_FEE_RATE: int = 25   # 0.25%
SHARE_FEE_RATE: int = 0       # 0%

# ============================================
# Instruction Discriminators
# ============================================

BUY_EXACT_IN_DISCRIMINATOR: bytes = bytes([250, 234, 13, 123, 213, 156, 19, 236])
SELL_EXACT_IN_DISCRIMINATOR: bytes = bytes([149, 39, 222, 155, 211, 124, 152, 26])

# ============================================
# Seeds
# ============================================

POOL_SEED = b"pool"
POOL_VAULT_SEED = b"pool_vault"

# ============================================
# PDA Derivation Functions
# ============================================

def get_pool_pda(base_mint: Pubkey, quote_mint: Pubkey) -> Pubkey:
    """
    Derive the pool PDA for a given base and quote mint.
    Seeds: ["pool", base_mint, quote_mint]
    """
    seeds = [POOL_SEED, bytes(base_mint), bytes(quote_mint)]
    (pda, _) = Pubkey.find_program_address(seeds, BONK_PROGRAM_ID)
    return pda


def get_vault_pda(pool_state: Pubkey, mint: Pubkey) -> Pubkey:
    """
    Derive the vault PDA for a given pool and mint.
    Seeds: ["pool_vault", pool_state, mint]
    """
    seeds = [POOL_VAULT_SEED, bytes(pool_state), bytes(mint)]
    (pda, _) = Pubkey.find_program_address(seeds, BONK_PROGRAM_ID)
    return pda


def get_platform_associated_account(platform_config: Pubkey) -> Pubkey:
    """
    Derive the platform associated account PDA.
    Seeds: [platform_config, WSOL_TOKEN_ACCOUNT]
    """
    seeds = [bytes(platform_config), bytes(WSOL_TOKEN_ACCOUNT)]
    (pda, _) = Pubkey.find_program_address(seeds, BONK_PROGRAM_ID)
    return pda


def get_creator_associated_account(creator: Pubkey) -> Pubkey:
    """
    Derive the creator associated account PDA.
    Seeds: [creator, WSOL_TOKEN_ACCOUNT]
    """
    seeds = [bytes(creator), bytes(WSOL_TOKEN_ACCOUNT)]
    (pda, _) = Pubkey.find_program_address(seeds, BONK_PROGRAM_ID)
    return pda


# ============================================
# Bonk Parameters Dataclass
# ============================================

@dataclass
class BonkParams:
    """Parameters for Bonk protocol trading."""
    virtual_base: int = 0
    virtual_quote: int = 0
    real_base: int = 0
    real_quote: int = 0
    pool_state: Optional[Pubkey] = None  # If None, will derive from mints
    base_vault: Optional[Pubkey] = None
    quote_vault: Optional[Pubkey] = None
    mint_token_program: Pubkey = TOKEN_PROGRAM
    platform_config: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    platform_associated_account: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    creator_associated_account: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    global_config: Pubkey = GLOBAL_CONFIG  # Use USD1_GLOBAL_CONFIG for USD1 pools

    @property
    def is_usd1_pool(self) -> bool:
        """Check if this is a USD1 pool."""
        return self.global_config == USD1_GLOBAL_CONFIG


# ============================================
# Bonk Calculation Functions
# ============================================

def get_amount_in_net(
    amount_in: int,
    protocol_fee_rate: int,
    platform_fee_rate: int,
    share_fee_rate: int,
) -> int:
    """Calculate net input amount after fees."""
    protocol_fee = (amount_in * protocol_fee_rate) // 10000
    platform_fee = (amount_in * platform_fee_rate) // 10000
    share_fee = (amount_in * share_fee_rate) // 10000
    return amount_in - protocol_fee - platform_fee - share_fee


def get_amount_out(
    amount_in: int,
    protocol_fee_rate: int,
    platform_fee_rate: int,
    share_fee_rate: int,
    virtual_base: int,
    virtual_quote: int,
    real_base: int,
    real_quote: int,
    slippage_bps: int,
) -> int:
    """
    Calculate output amount for a given input amount on Bonk.
    """
    amount_in_net = get_amount_in_net(
        amount_in, protocol_fee_rate, platform_fee_rate, share_fee_rate
    )

    input_reserve = virtual_quote + real_quote
    output_reserve = virtual_base - real_base

    numerator = amount_in_net * output_reserve
    denominator = input_reserve + amount_in_net
    amount_out = numerator // denominator

    # Apply slippage
    amount_out = amount_out - (amount_out * slippage_bps) // 10000
    return amount_out


def get_buy_token_amount_from_sol_amount(
    amount_in: int,
    virtual_base: int,
    virtual_quote: int,
    real_base: int,
    real_quote: int,
    slippage_bps: int,
) -> int:
    """
    Calculate the token amount received for a given SOL amount on Bonk.
    """
    return get_amount_out(
        amount_in,
        PROTOCOL_FEE_RATE,
        PLATFORM_FEE_RATE,
        SHARE_FEE_RATE,
        virtual_base,
        virtual_quote,
        real_base,
        real_quote,
        slippage_bps,
    )


def get_sell_sol_amount_from_token_amount(
    amount_in: int,
    virtual_base: int,
    virtual_quote: int,
    real_base: int,
    real_quote: int,
    slippage_bps: int,
) -> int:
    """
    Calculate the SOL amount received for a given token amount on Bonk.
    For sell, we swap base -> quote.
    """
    # For sell: base is input, quote is output
    # So we swap virtual_base with virtual_quote roles
    return get_amount_out(
        amount_in,
        PROTOCOL_FEE_RATE,
        PLATFORM_FEE_RATE,
        SHARE_FEE_RATE,
        virtual_quote,  # Swapped
        virtual_base,   # Swapped
        real_quote,     # Swapped
        real_base,      # Swapped
        slippage_bps,
    )


# ============================================
# Build Buy Instructions
# ============================================

def build_buy_instructions(
    payer: Pubkey,
    output_mint: Pubkey,
    input_amount: int,
    params: BonkParams,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_input_ata: bool = True,
    create_output_ata: bool = True,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build Bonk buy instructions.

    Args:
        payer: The wallet paying for the swap
        output_mint: The token mint to buy
        input_amount: Amount of SOL/quote to spend
        params: Bonk protocol parameters
        slippage_bps: Slippage tolerance in basis points
        create_input_ata: Whether to create WSOL ATA if needed
        create_output_ata: Whether to create output token ATA if needed
        close_input_ata: Whether to close WSOL ATA after swap
        fixed_output_amount: If set, use this as exact output amount

    Returns:
        List of instructions for the buy operation
    """
    if input_amount == 0:
        raise ValueError("Amount cannot be zero")

    instructions = []
    is_usd1_pool = params.is_usd1_pool

    # Determine quote mint (WSOL or USD1)
    quote_mint = USD1_TOKEN_ACCOUNT if is_usd1_pool else WSOL_TOKEN_ACCOUNT
    quote_token_mint_meta = AccountMeta(quote_mint, False, False)

    # Get pool state
    pool_state = params.pool_state
    if pool_state is None:
        pool_state = get_pool_pda(output_mint, quote_mint)

    # Get global config
    global_config = USD1_GLOBAL_CONFIG if is_usd1_pool else GLOBAL_CONFIG
    global_config_meta = AccountMeta(global_config, False, False)

    # Calculate minimum output amount
    if fixed_output_amount is not None:
        minimum_amount_out = fixed_output_amount
    else:
        minimum_amount_out = get_buy_token_amount_from_sol_amount(
            input_amount,
            params.virtual_base,
            params.virtual_quote,
            params.real_base,
            params.real_quote,
            slippage_bps,
        )

    share_fee_rate = 0

    # Get user token accounts
    user_base_token_account = get_associated_token_address(
        payer, output_mint, params.mint_token_program
    )
    user_quote_token_account = get_associated_token_address(
        payer, quote_mint, TOKEN_PROGRAM
    )

    # Get vaults
    base_vault = params.base_vault
    if base_vault is None:
        base_vault = get_vault_pda(pool_state, output_mint)

    quote_vault = params.quote_vault
    if quote_vault is None:
        quote_vault = get_vault_pda(pool_state, quote_mint)

    # Handle WSOL for non-USD1 pools
    if create_input_ata and not is_usd1_pool:
        instructions.extend(handle_wsol(payer, input_amount))

    # Create output ATA if needed
    if create_output_ata:
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, output_mint, params.mint_token_program
            )
        )

    # Build instruction data
    data = BUY_EXACT_IN_DISCRIMINATOR + struct.pack("<QQQ", input_amount, minimum_amount_out, share_fee_rate)

    # Build accounts list
    accounts = [
        AccountMeta(payer, True, True),  # payer (signer, writable)
        AccountMeta(AUTHORITY, False, False),  # authority (readonly)
        global_config_meta,  # global_config (readonly)
        AccountMeta(params.platform_config, False, False),  # platform_config (readonly)
        AccountMeta(pool_state, False, True),  # pool_state (writable)
        AccountMeta(user_base_token_account, False, True),  # user_base_token_account (writable)
        AccountMeta(user_quote_token_account, False, True),  # user_quote_token_account (writable)
        AccountMeta(base_vault, False, True),  # base_vault (writable)
        AccountMeta(quote_vault, False, True),  # quote_vault (writable)
        AccountMeta(output_mint, False, False),  # base_token_mint (readonly)
        quote_token_mint_meta,  # quote_token_mint (readonly)
        AccountMeta(params.mint_token_program, False, False),  # base_token_program (readonly)
        AccountMeta(TOKEN_PROGRAM, False, False),  # quote_token_program (readonly)
        AccountMeta(EVENT_AUTHORITY, False, False),  # event_authority (readonly)
        AccountMeta(BONK_PROGRAM_ID, False, False),  # program (readonly)
        AccountMeta(SYSTEM_PROGRAM, False, False),  # system_program (readonly)
        AccountMeta(params.platform_associated_account, False, True),  # platform_associated_account (writable)
        AccountMeta(params.creator_associated_account, False, True),  # creator_associated_account (writable)
    ]

    instructions.append(Instruction(BONK_PROGRAM_ID, data, accounts))

    # Close WSOL ATA if requested
    if close_input_ata and not is_usd1_pool:
        instructions.extend(close_wsol(payer))

    return instructions


# ============================================
# Build Sell Instructions
# ============================================

def build_sell_instructions(
    payer: Pubkey,
    input_mint: Pubkey,
    input_amount: int,
    params: BonkParams,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_output_ata: bool = True,
    close_output_ata: bool = False,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build Bonk sell instructions.

    Args:
        payer: The wallet paying for the swap
        input_mint: The token mint to sell
        input_amount: Amount of tokens to sell
        params: Bonk protocol parameters
        slippage_bps: Slippage tolerance in basis points
        create_output_ata: Whether to create WSOL ATA for receiving SOL
        close_output_ata: Whether to close WSOL ATA after swap
        close_input_ata: Whether to close token ATA after swap
        fixed_output_amount: If set, use this as exact output amount

    Returns:
        List of instructions for the sell operation
    """
    if input_amount == 0:
        raise ValueError("Amount cannot be zero")

    instructions = []
    is_usd1_pool = params.is_usd1_pool

    # Determine quote mint (WSOL or USD1)
    quote_mint = USD1_TOKEN_ACCOUNT if is_usd1_pool else WSOL_TOKEN_ACCOUNT
    quote_token_mint_meta = AccountMeta(quote_mint, False, False)

    # Get pool state
    pool_state = params.pool_state
    if pool_state is None:
        pool_state = get_pool_pda(input_mint, quote_mint)

    # Get global config
    global_config = USD1_GLOBAL_CONFIG if is_usd1_pool else GLOBAL_CONFIG
    global_config_meta = AccountMeta(global_config, False, False)

    # Calculate minimum output amount
    share_fee_rate = 0
    if fixed_output_amount is not None:
        minimum_amount_out = fixed_output_amount
    else:
        minimum_amount_out = get_sell_sol_amount_from_token_amount(
            input_amount,
            params.virtual_base,
            params.virtual_quote,
            params.real_base,
            params.real_quote,
            slippage_bps,
        )

    # Get user token accounts
    user_base_token_account = get_associated_token_address(
        payer, input_mint, params.mint_token_program
    )
    user_quote_token_account = get_associated_token_address(
        payer, quote_mint, TOKEN_PROGRAM
    )

    # Get vaults
    base_vault = params.base_vault
    if base_vault is None:
        base_vault = get_vault_pda(pool_state, input_mint)

    quote_vault = params.quote_vault
    if quote_vault is None:
        quote_vault = get_vault_pda(pool_state, quote_mint)

    # Create WSOL ATA if needed for receiving SOL
    if create_output_ata and not is_usd1_pool:
        wsol_ata = get_associated_token_address(payer, WSOL_TOKEN_ACCOUNT, TOKEN_PROGRAM)
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, WSOL_TOKEN_ACCOUNT, TOKEN_PROGRAM
            )
        )

    # Build instruction data
    data = SELL_EXACT_IN_DISCRIMINATOR + struct.pack("<QQQ", input_amount, minimum_amount_out, share_fee_rate)

    # Build accounts list
    accounts = [
        AccountMeta(payer, True, True),  # payer (signer, writable)
        AccountMeta(AUTHORITY, False, False),  # authority (readonly)
        global_config_meta,  # global_config (readonly)
        AccountMeta(params.platform_config, False, False),  # platform_config (readonly)
        AccountMeta(pool_state, False, True),  # pool_state (writable)
        AccountMeta(user_base_token_account, False, True),  # user_base_token_account (writable)
        AccountMeta(user_quote_token_account, False, True),  # user_quote_token_account (writable)
        AccountMeta(base_vault, False, True),  # base_vault (writable)
        AccountMeta(quote_vault, False, True),  # quote_vault (writable)
        AccountMeta(input_mint, False, False),  # base_token_mint (readonly)
        quote_token_mint_meta,  # quote_token_mint (readonly)
        AccountMeta(params.mint_token_program, False, False),  # base_token_program (readonly)
        AccountMeta(TOKEN_PROGRAM, False, False),  # quote_token_program (readonly)
        AccountMeta(EVENT_AUTHORITY, False, False),  # event_authority (readonly)
        AccountMeta(BONK_PROGRAM_ID, False, False),  # program (readonly)
        AccountMeta(SYSTEM_PROGRAM, False, False),  # system_program (readonly)
        AccountMeta(params.platform_associated_account, False, True),  # platform_associated_account (writable)
        AccountMeta(params.creator_associated_account, False, True),  # creator_associated_account (writable)
    ]

    instructions.append(Instruction(BONK_PROGRAM_ID, data, accounts))

    # Close WSOL ATA if requested
    if close_output_ata and not is_usd1_pool:
        instructions.extend(close_wsol(payer))

    # Close token ATA if requested
    if close_input_ata:
        instructions.append(
            close_token_account_instruction(
                params.mint_token_program,
                user_base_token_account,
                payer,
                payer,
            )
        )

    return instructions


# ============================================
# Exports
# ============================================

__all__ = [
    # Program IDs and Constants
    "BONK_PROGRAM_ID",
    "AUTHORITY",
    "GLOBAL_CONFIG",
    "USD1_GLOBAL_CONFIG",
    "EVENT_AUTHORITY",
    "PLATFORM_FEE_RATE",
    "PROTOCOL_FEE_RATE",
    "SHARE_FEE_RATE",
    # Discriminators
    "BUY_EXACT_IN_DISCRIMINATOR",
    "SELL_EXACT_IN_DISCRIMINATOR",
    # PDA Functions
    "get_pool_pda",
    "get_vault_pda",
    "get_platform_associated_account",
    "get_creator_associated_account",
    # Params
    "BonkParams",
    # Calculation Functions
    "get_amount_in_net",
    "get_amount_out",
    "get_buy_token_amount_from_sol_amount",
    "get_sell_sol_amount_from_token_amount",
    # Instruction Builders
    "build_buy_instructions",
    "build_sell_instructions",
]
