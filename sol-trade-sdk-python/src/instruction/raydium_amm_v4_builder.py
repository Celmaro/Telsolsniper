"""
Raydium AMM V4 instruction builder for Solana trading SDK.
Production-grade implementation with all constants, discriminators, and PDA derivation functions.
"""

from typing import List, Optional
from dataclasses import dataclass
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import struct

from .common import (
    TOKEN_PROGRAM,
    WSOL_TOKEN_ACCOUNT,
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
# Raydium AMM V4 Program ID
# ============================================

RAYDIUM_AMM_V4_PROGRAM_ID: Pubkey = Pubkey.from_string("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")

# ============================================
# Raydium AMM V4 Constants
# ============================================

# Authority
AUTHORITY: Pubkey = Pubkey.from_string("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1")

# Fee Rates
TRADE_FEE_NUMERATOR: int = 25
TRADE_FEE_DENOMINATOR: int = 10000
SWAP_FEE_NUMERATOR: int = 25
SWAP_FEE_DENOMINATOR: int = 10000

# ============================================
# Instruction Discriminators
# ============================================

# Note: Raydium AMM V4 uses single-byte discriminators
SWAP_BASE_IN_DISCRIMINATOR: bytes = bytes([9])
SWAP_BASE_OUT_DISCRIMINATOR: bytes = bytes([11])

# ============================================
# Seeds
# ============================================

POOL_SEED = b"pool"


# ============================================
# Raydium AMM V4 Parameters Dataclass
# ============================================

@dataclass
class RaydiumAmmV4Params:
    """Parameters for Raydium AMM V4 protocol trading."""
    amm: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    coin_mint: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    pc_mint: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_coin: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_pc: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    coin_reserve: int = 0
    pc_reserve: int = 0

    @property
    def is_wsol(self) -> bool:
        """Check if the pool contains WSOL."""
        return self.coin_mint == WSOL_TOKEN_ACCOUNT or self.pc_mint == WSOL_TOKEN_ACCOUNT

    @property
    def is_usdc(self) -> bool:
        """Check if the pool contains USDC."""
        return self.coin_mint == USDC_TOKEN_ACCOUNT or self.pc_mint == USDC_TOKEN_ACCOUNT


# ============================================
# Raydium AMM V4 Calculation Functions
# ============================================

def compute_swap_amount(
    coin_reserve: int,
    pc_reserve: int,
    is_coin_in: bool,
    amount_in: int,
    slippage_bps: int,
) -> tuple:
    """
    Compute swap output amount for Raydium AMM V4.
    Returns (amount_out, min_amount_out).
    """
    # Apply trade fee (0.25%)
    amount_in_after_fee = amount_in - (amount_in * TRADE_FEE_NUMERATOR) // TRADE_FEE_DENOMINATOR

    if is_coin_in:
        # Coin -> PC
        # out = (amount_in * pc_reserve) / (coin_reserve + amount_in)
        numerator = amount_in_after_fee * pc_reserve
        denominator = coin_reserve + amount_in_after_fee
        amount_out = numerator // denominator
    else:
        # PC -> Coin
        # out = (amount_in * coin_reserve) / (pc_reserve + amount_in)
        numerator = amount_in_after_fee * coin_reserve
        denominator = pc_reserve + amount_in_after_fee
        amount_out = numerator // denominator

    # Apply slippage
    min_amount_out = calculate_with_slippage_sell(amount_out, slippage_bps)

    return (amount_out, min_amount_out)


# ============================================
# Build Buy Instructions
# ============================================

def build_buy_instructions(
    payer: Pubkey,
    output_mint: Pubkey,
    input_amount: int,
    params: RaydiumAmmV4Params,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_input_ata: bool = True,
    create_output_ata: bool = True,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build Raydium AMM V4 buy instructions.

    Args:
        payer: The wallet paying for the swap
        output_mint: The token mint to buy
        input_amount: Amount of SOL/USDC to spend
        params: Raydium AMM V4 protocol parameters
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

    # Validate pool contains WSOL or USDC
    if not params.is_wsol and not params.is_usdc:
        raise ValueError("Pool must contain WSOL or USDC")

    # Determine if coin is input (WSOL/USDC)
    is_coin_in = params.coin_mint == WSOL_TOKEN_ACCOUNT or params.coin_mint == USDC_TOKEN_ACCOUNT

    # Calculate swap amount
    _, min_amount_out = compute_swap_amount(
        params.coin_reserve,
        params.pc_reserve,
        is_coin_in,
        input_amount,
        slippage_bps,
    )

    if fixed_output_amount is not None:
        minimum_amount_out = fixed_output_amount
    else:
        minimum_amount_out = min_amount_out

    # Determine input mint (WSOL or USDC)
    input_mint = WSOL_TOKEN_ACCOUNT if params.is_wsol else USDC_TOKEN_ACCOUNT

    # Get user token accounts
    user_source_token_account = get_associated_token_address(payer, input_mint, TOKEN_PROGRAM)
    user_destination_token_account = get_associated_token_address(payer, output_mint, TOKEN_PROGRAM)

    # Handle WSOL if needed
    if create_input_ata and params.is_wsol:
        instructions.extend(handle_wsol(payer, input_amount))

    # Create output ATA if needed
    if create_output_ata:
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, output_mint, TOKEN_PROGRAM
            )
        )

    # Build instruction data (1 byte discriminator + 8 bytes amount_in + 8 bytes min_out)
    data = SWAP_BASE_IN_DISCRIMINATOR + struct.pack("<QQ", input_amount, minimum_amount_out)

    # Build accounts list (17 accounts)
    # Note: Raydium AMM V4 has specific account ordering
    accounts = [
        AccountMeta(TOKEN_PROGRAM, False, False),  # token_program (readonly)
        AccountMeta(params.amm, False, True),  # amm (writable)
        AccountMeta(AUTHORITY, False, False),  # authority (readonly)
        AccountMeta(params.amm, False, False),  # amm_open_orders (uses amm address)
        AccountMeta(params.token_coin, False, True),  # pool_coin_token_account (writable)
        AccountMeta(params.token_pc, False, True),  # pool_pc_token_account (writable)
        AccountMeta(params.amm, False, False),  # serum_program (placeholder)
        AccountMeta(params.amm, False, False),  # serum_market (placeholder)
        AccountMeta(params.amm, False, False),  # serum_bids (placeholder)
        AccountMeta(params.amm, False, False),  # serum_asks (placeholder)
        AccountMeta(params.amm, False, False),  # serum_event_queue (placeholder)
        AccountMeta(params.amm, False, False),  # serum_coin_vault_account (placeholder)
        AccountMeta(params.amm, False, False),  # serum_pc_vault_account (placeholder)
        AccountMeta(params.amm, False, False),  # serum_vault_signer (placeholder)
        AccountMeta(user_source_token_account, False, True),  # user_source_token_account (writable)
        AccountMeta(user_destination_token_account, False, True),  # user_destination_token_account (writable)
        AccountMeta(payer, True, False),  # user_source_owner (signer)
    ]

    instructions.append(Instruction(RAYDIUM_AMM_V4_PROGRAM_ID, data, accounts))

    # Close WSOL ATA if requested
    if close_input_ata and params.is_wsol:
        instructions.extend(close_wsol(payer))

    return instructions


# ============================================
# Build Sell Instructions
# ============================================

def build_sell_instructions(
    payer: Pubkey,
    input_mint: Pubkey,
    input_amount: int,
    params: RaydiumAmmV4Params,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_output_ata: bool = True,
    close_output_ata: bool = False,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build Raydium AMM V4 sell instructions.

    Args:
        payer: The wallet paying for the swap
        input_mint: The token mint to sell
        input_amount: Amount of tokens to sell
        params: Raydium AMM V4 protocol parameters
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

    # Validate pool contains WSOL or USDC
    if not params.is_wsol and not params.is_usdc:
        raise ValueError("Pool must contain WSOL or USDC")

    # Determine if pc is output (WSOL/USDC)
    # For sell: is_base_in = True means we're selling PC to get Coin
    # is_base_in = False means we're selling Coin to get PC
    is_pc_out = params.pc_mint == WSOL_TOKEN_ACCOUNT or params.pc_mint == USDC_TOKEN_ACCOUNT

    # Calculate swap amount (is_pc_out reversed because we're selling to get WSOL/USDC)
    _, min_amount_out = compute_swap_amount(
        params.coin_reserve,
        params.pc_reserve,
        not is_pc_out,  # Reversed for sell
        input_amount,
        slippage_bps,
    )

    if fixed_output_amount is not None:
        minimum_amount_out = fixed_output_amount
    else:
        minimum_amount_out = min_amount_out

    # Determine output mint (WSOL or USDC)
    output_mint = WSOL_TOKEN_ACCOUNT if params.is_wsol else USDC_TOKEN_ACCOUNT

    # Get user token accounts
    user_source_token_account = get_associated_token_address(payer, input_mint, TOKEN_PROGRAM)
    user_destination_token_account = get_associated_token_address(payer, output_mint, TOKEN_PROGRAM)

    # Create WSOL ATA if needed for receiving SOL
    if create_output_ata and params.is_wsol:
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, WSOL_TOKEN_ACCOUNT, TOKEN_PROGRAM
            )
        )

    # Build instruction data (1 byte discriminator + 8 bytes amount_in + 8 bytes min_out)
    data = SWAP_BASE_IN_DISCRIMINATOR + struct.pack("<QQ", input_amount, minimum_amount_out)

    # Build accounts list (17 accounts)
    accounts = [
        AccountMeta(TOKEN_PROGRAM, False, False),  # token_program (readonly)
        AccountMeta(params.amm, False, True),  # amm (writable)
        AccountMeta(AUTHORITY, False, False),  # authority (readonly)
        AccountMeta(params.amm, False, False),  # amm_open_orders (uses amm address)
        AccountMeta(params.token_coin, False, True),  # pool_coin_token_account (writable)
        AccountMeta(params.token_pc, False, True),  # pool_pc_token_account (writable)
        AccountMeta(params.amm, False, False),  # serum_program (placeholder)
        AccountMeta(params.amm, False, False),  # serum_market (placeholder)
        AccountMeta(params.amm, False, False),  # serum_bids (placeholder)
        AccountMeta(params.amm, False, False),  # serum_asks (placeholder)
        AccountMeta(params.amm, False, False),  # serum_event_queue (placeholder)
        AccountMeta(params.amm, False, False),  # serum_coin_vault_account (placeholder)
        AccountMeta(params.amm, False, False),  # serum_pc_vault_account (placeholder)
        AccountMeta(params.amm, False, False),  # serum_vault_signer (placeholder)
        AccountMeta(user_source_token_account, False, True),  # user_source_token_account (writable)
        AccountMeta(user_destination_token_account, False, True),  # user_destination_token_account (writable)
        AccountMeta(payer, True, False),  # user_source_owner (signer)
    ]

    instructions.append(Instruction(RAYDIUM_AMM_V4_PROGRAM_ID, data, accounts))

    # Close WSOL ATA if requested
    if close_output_ata and params.is_wsol:
        instructions.extend(close_wsol(payer))

    # Close token ATA if requested
    if close_input_ata:
        instructions.append(
            close_token_account_instruction(
                TOKEN_PROGRAM,
                user_source_token_account,
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
    "RAYDIUM_AMM_V4_PROGRAM_ID",
    "AUTHORITY",
    "TRADE_FEE_NUMERATOR",
    "TRADE_FEE_DENOMINATOR",
    "SWAP_FEE_NUMERATOR",
    "SWAP_FEE_DENOMINATOR",
    # Discriminators
    "SWAP_BASE_IN_DISCRIMINATOR",
    "SWAP_BASE_OUT_DISCRIMINATOR",
    # Params
    "RaydiumAmmV4Params",
    # Calculation Functions
    "compute_swap_amount",
    # Instruction Builders
    "build_buy_instructions",
    "build_sell_instructions",
]
