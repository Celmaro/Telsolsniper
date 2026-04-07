"""
Meteora DAMM V2 instruction builder for Solana trading SDK.
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
)

# ============================================
# Meteora DAMM V2 Program ID
# ============================================

METEORA_DAMM_V2_PROGRAM_ID: Pubkey = Pubkey.from_string("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG")

# ============================================
# Meteora DAMM V2 Constants
# ============================================

# Pool Authority
AUTHORITY: Pubkey = Pubkey.from_string("HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC")

# ============================================
# Instruction Discriminators
# ============================================

SWAP_DISCRIMINATOR: bytes = bytes([248, 198, 158, 145, 225, 117, 135, 200])

# ============================================
# Seeds
# ============================================

EVENT_AUTHORITY_SEED = b"__event_authority"


# ============================================
# PDA Derivation Functions
# ============================================

def get_event_authority_pda() -> Pubkey:
    """
    Derive the event authority PDA.
    Seeds: ["__event_authority"]
    """
    seeds = [EVENT_AUTHORITY_SEED]
    (pda, _) = Pubkey.find_program_address(seeds, METEORA_DAMM_V2_PROGRAM_ID)
    return pda


# ============================================
# Meteora DAMM V2 Parameters Dataclass
# ============================================

@dataclass
class MeteoraDammV2Params:
    """Parameters for Meteora DAMM V2 protocol trading."""
    pool: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_a_vault: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_b_vault: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_a_mint: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_b_mint: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    token_a_program: Pubkey = TOKEN_PROGRAM
    token_b_program: Pubkey = TOKEN_PROGRAM

    @property
    def is_wsol(self) -> bool:
        """Check if the pool contains WSOL."""
        return self.token_a_mint == WSOL_TOKEN_ACCOUNT or self.token_b_mint == WSOL_TOKEN_ACCOUNT

    @property
    def is_usdc(self) -> bool:
        """Check if the pool contains USDC."""
        return self.token_a_mint == USDC_TOKEN_ACCOUNT or self.token_b_mint == USDC_TOKEN_ACCOUNT


# ============================================
# Build Buy Instructions
# ============================================

def build_buy_instructions(
    payer: Pubkey,
    output_mint: Pubkey,
    input_amount: int,
    params: MeteoraDammV2Params,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_input_ata: bool = True,
    create_output_ata: bool = True,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build Meteora DAMM V2 buy instructions.

    Args:
        payer: The wallet paying for the swap
        output_mint: The token mint to buy
        input_amount: Amount of SOL/USDC to spend
        params: Meteora DAMM V2 protocol parameters
        slippage_bps: Slippage tolerance in basis points
        create_input_ata: Whether to create WSOL ATA if needed
        create_output_ata: Whether to create output token ATA if needed
        close_input_ata: Whether to close WSOL ATA after swap
        fixed_output_amount: MUST be set for Meteora DAMM V2 swaps

    Returns:
        List of instructions for the buy operation
    """
    if input_amount == 0:
        raise ValueError("Amount cannot be zero")

    instructions = []

    # Validate pool contains WSOL or USDC
    if not params.is_wsol and not params.is_usdc:
        raise ValueError("Pool must contain WSOL or USDC")

    # Determine if token A is input (WSOL/USDC)
    is_a_in = params.token_a_mint == WSOL_TOKEN_ACCOUNT or params.token_a_mint == USDC_TOKEN_ACCOUNT

    # Meteora DAMM V2 requires fixed_output_amount
    if fixed_output_amount is None:
        raise ValueError("fixed_output_amount must be set for MeteoraDammV2 swap")

    minimum_amount_out = fixed_output_amount

    # Determine input/output mints and programs
    input_mint = WSOL_TOKEN_ACCOUNT if params.is_wsol else USDC_TOKEN_ACCOUNT

    input_token_program = params.token_a_program if is_a_in else params.token_b_program
    output_token_program = params.token_b_program if is_a_in else params.token_a_program

    # Get user token accounts
    input_token_account = get_associated_token_address(payer, input_mint, TOKEN_PROGRAM)
    output_token_account = get_associated_token_address(payer, output_mint, TOKEN_PROGRAM)

    # Get event authority PDA
    event_authority = get_event_authority_pda()

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

    # Build instruction data
    data = SWAP_DISCRIMINATOR + struct.pack("<QQ", input_amount, minimum_amount_out)

    # Build accounts list (14 accounts)
    accounts = [
        AccountMeta(AUTHORITY, False, False),  # pool_authority (readonly)
        AccountMeta(params.pool, False, True),  # pool (writable)
        AccountMeta(input_token_account, False, True),  # input_token_account (writable)
        AccountMeta(output_token_account, False, True),  # output_token_account (writable)
        AccountMeta(params.token_a_vault, False, True),  # token_a_vault (writable)
        AccountMeta(params.token_b_vault, False, True),  # token_b_vault (writable)
        AccountMeta(params.token_a_mint, False, False),  # token_a_mint (readonly)
        AccountMeta(params.token_b_mint, False, False),  # token_b_mint (readonly)
        AccountMeta(payer, True, False),  # user_transfer_authority (signer)
        AccountMeta(params.token_a_program, False, False),  # token_a_program (readonly)
        AccountMeta(params.token_b_program, False, False),  # token_b_program (readonly)
        AccountMeta(METEORA_DAMM_V2_PROGRAM_ID, False, False),  # referral_token_account (readonly, program)
        AccountMeta(event_authority, False, False),  # event_authority (readonly)
        AccountMeta(METEORA_DAMM_V2_PROGRAM_ID, False, False),  # program (readonly)
    ]

    instructions.append(Instruction(METEORA_DAMM_V2_PROGRAM_ID, data, accounts))

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
    params: MeteoraDammV2Params,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_output_ata: bool = True,
    close_output_ata: bool = False,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build Meteora DAMM V2 sell instructions.

    Args:
        payer: The wallet paying for the swap
        input_mint: The token mint to sell
        input_amount: Amount of tokens to sell
        params: Meteora DAMM V2 protocol parameters
        slippage_bps: Slippage tolerance in basis points
        create_output_ata: Whether to create WSOL ATA for receiving SOL
        close_output_ata: Whether to close WSOL ATA after swap
        close_input_ata: Whether to close token ATA after swap
        fixed_output_amount: MUST be set for Meteora DAMM V2 swaps

    Returns:
        List of instructions for the sell operation
    """
    if input_amount == 0:
        raise ValueError("Amount cannot be zero")

    instructions = []

    # Validate pool contains WSOL or USDC
    if not params.is_wsol and not params.is_usdc:
        raise ValueError("Pool must contain WSOL or USDC")

    # Determine if token B is output (WSOL/USDC)
    is_a_in = params.token_b_mint == WSOL_TOKEN_ACCOUNT or params.token_b_mint == USDC_TOKEN_ACCOUNT

    # Meteora DAMM V2 requires fixed_output_amount
    if fixed_output_amount is None:
        raise ValueError("fixed_output_amount must be set for MeteoraDammV2 swap")

    minimum_amount_out = fixed_output_amount

    # Determine output mint (WSOL or USDC)
    output_mint = WSOL_TOKEN_ACCOUNT if params.is_wsol else USDC_TOKEN_ACCOUNT

    # Get token programs based on direction
    input_token_program = params.token_a_program if is_a_in else params.token_b_program
    output_token_program = params.token_b_program if is_a_in else params.token_a_program

    # Get user token accounts
    input_token_account = get_associated_token_address(payer, input_mint, input_token_program)
    output_token_account = get_associated_token_address(payer, output_mint, TOKEN_PROGRAM)

    # Get event authority PDA
    event_authority = get_event_authority_pda()

    # Create WSOL ATA if needed for receiving SOL
    if create_output_ata and params.is_wsol:
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, WSOL_TOKEN_ACCOUNT, TOKEN_PROGRAM
            )
        )

    # Build instruction data
    data = SWAP_DISCRIMINATOR + struct.pack("<QQ", input_amount, minimum_amount_out)

    # Build accounts list (14 accounts)
    accounts = [
        AccountMeta(AUTHORITY, False, False),  # pool_authority (readonly)
        AccountMeta(params.pool, False, True),  # pool (writable)
        AccountMeta(input_token_account, False, True),  # input_token_account (writable)
        AccountMeta(output_token_account, False, True),  # output_token_account (writable)
        AccountMeta(params.token_a_vault, False, True),  # token_a_vault (writable)
        AccountMeta(params.token_b_vault, False, True),  # token_b_vault (writable)
        AccountMeta(params.token_a_mint, False, False),  # token_a_mint (readonly)
        AccountMeta(params.token_b_mint, False, False),  # token_b_mint (readonly)
        AccountMeta(payer, True, False),  # user_transfer_authority (signer)
        AccountMeta(params.token_a_program, False, False),  # token_a_program (readonly)
        AccountMeta(params.token_b_program, False, False),  # token_b_program (readonly)
        AccountMeta(METEORA_DAMM_V2_PROGRAM_ID, False, False),  # referral_token_account (readonly, program)
        AccountMeta(event_authority, False, False),  # event_authority (readonly)
        AccountMeta(METEORA_DAMM_V2_PROGRAM_ID, False, False),  # program (readonly)
    ]

    instructions.append(Instruction(METEORA_DAMM_V2_PROGRAM_ID, data, accounts))

    # Close WSOL ATA if requested
    if close_output_ata and params.is_wsol:
        instructions.extend(close_wsol(payer))

    # Close token ATA if requested
    if close_input_ata:
        instructions.append(
            close_token_account_instruction(
                input_token_program,
                input_token_account,
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
    "METEORA_DAMM_V2_PROGRAM_ID",
    "AUTHORITY",
    # Discriminators
    "SWAP_DISCRIMINATOR",
    # PDA Functions
    "get_event_authority_pda",
    # Params
    "MeteoraDammV2Params",
    # Instruction Builders
    "build_buy_instructions",
    "build_sell_instructions",
]
