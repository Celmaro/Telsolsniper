"""
PumpSwap instruction builder for Solana trading SDK.
Production-grade implementation with all constants, discriminators, and PDA derivation functions.
"""

from typing import List, Optional, Tuple
from dataclasses import dataclass
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import struct
import random

from .common import (
    SYSTEM_PROGRAM,
    TOKEN_PROGRAM,
    TOKEN_PROGRAM_2022,
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
# PumpSwap Program ID
# ============================================

PUMPSWAP_PROGRAM_ID: Pubkey = Pubkey.from_string("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")

# ============================================
# PumpSwap Constants
# ============================================

# Fee Recipient
FEE_RECIPIENT: Pubkey = Pubkey.from_string("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV")

# Global Account
GLOBAL_ACCOUNT: Pubkey = Pubkey.from_string("ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw")

# Event Authority
EVENT_AUTHORITY: Pubkey = Pubkey.from_string("GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR")

# Associated Token Program
ASSOCIATED_TOKEN_PROGRAM: Pubkey = Pubkey.from_string("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")

# Protocol Fee Recipient
PROTOCOL_FEE_RECIPIENT: Pubkey = Pubkey.from_string("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV")

# Pump Program ID (Bonding Curve)
PUMP_PROGRAM_ID: Pubkey = Pubkey.from_string("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P")

# Fee Program
FEE_PROGRAM: Pubkey = Pubkey.from_string("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ")

# Global Volume Accumulator
GLOBAL_VOLUME_ACCUMULATOR: Pubkey = Pubkey.from_string("C2aFPdENg4A2HQsmrd5rTw5TaYBX5Ku887cWjbFKtZpw")

# Fee Config
FEE_CONFIG: Pubkey = Pubkey.from_string("5PHirr8joyTMp9JMm6N7hNDVyEYdkzDqazxPD7RaTjx")

# Default Coin Creator Vault Authority
DEFAULT_COIN_CREATOR_VAULT_AUTHORITY: Pubkey = Pubkey.from_string("8N3GDaZ2iwN65oxVatKTLPNooAVUJTbfiVJ1ahyqwjSk")

# Mayhem Fee Recipients
MAYHEM_FEE_RECIPIENTS: List[Pubkey] = [
    Pubkey.from_string("GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS"),
    Pubkey.from_string("4budycTjhs9fD6xw62VBducVTNgMgJJ5BgtKq7mAZwn6"),
    Pubkey.from_string("8SBKzEQU4nLSzcwF4a74F2iaUDQyTfjGndn6qUWBnrpR"),
    Pubkey.from_string("4UQeTP1T39KZ9Sfxzo3WR5skgsaP6NZa87BAkuazLEKH"),
    Pubkey.from_string("8sNeir4QsLsJdYpc9RZacohhK1Y5FLU3nC5LXgYB4aa6"),
    Pubkey.from_string("Fh9HmeLNUMVCvejxCtCL2DbYaRyBFVJ5xrWkLnMH6fdk"),
    Pubkey.from_string("463MEnMeGyJekNZFQSTUABBEbLnvMTALbT6ZmsxAbAdq"),
    Pubkey.from_string("6AUH3WEHucYZyC61hqpqYUWVto5qA5hjHuNQ32GNnNxA"),
]

# Fee Basis Points
LP_FEE_BASIS_POINTS: int = 25
PROTOCOL_FEE_BASIS_POINTS: int = 5
COIN_CREATOR_FEE_BASIS_POINTS: int = 5

# ============================================
# Instruction Discriminators
# ============================================

BUY_DISCRIMINATOR: bytes = bytes([102, 6, 61, 18, 1, 218, 235, 234])
BUY_EXACT_QUOTE_IN_DISCRIMINATOR: bytes = bytes([198, 46, 21, 82, 180, 217, 232, 112])
SELL_DISCRIMINATOR: bytes = bytes([51, 230, 133, 164, 1, 127, 131, 173])
CLAIM_CASHBACK_DISCRIMINATOR: bytes = bytes([37, 58, 35, 126, 190, 53, 228, 197])

# ============================================
# Seeds
# ============================================

GLOBAL_SEED = b"global"
MINT_AUTHORITY_SEED = b"mint-authority"
BONDING_CURVE_SEED = b"bonding-curve"
METADATA_SEED = b"metadata"
USER_VOLUME_ACCUMULATOR_SEED = b"user_volume_accumulator"
GLOBAL_VOLUME_ACCUMULATOR_SEED = b"global_volume_accumulator"
FEE_CONFIG_SEED = b"fee_config"
POOL_V2_SEED = b"pool-v2"
POOL_SEED = b"pool"
POOL_AUTHORITY_SEED = b"pool-authority"

# ============================================
# PDA Derivation Functions
# ============================================

def get_pool_v2_pda(base_mint: Pubkey) -> Pubkey:
    """
    Derive the pool v2 PDA for a given base mint.
    Seeds: ["pool-v2", base_mint]
    """
    seeds = [POOL_V2_SEED, bytes(base_mint)]
    (pda, _) = Pubkey.find_program_address(seeds, PUMPSWAP_PROGRAM_ID)
    return pda


def get_pump_pool_authority_pda(mint: Pubkey) -> Pubkey:
    """
    Derive the pump pool authority PDA for canonical pool.
    Seeds: ["pool-authority", mint]
    """
    seeds = [POOL_AUTHORITY_SEED, bytes(mint)]
    (pda, _) = Pubkey.find_program_address(seeds, PUMP_PROGRAM_ID)
    return pda


def get_canonical_pool_pda(mint: Pubkey) -> Pubkey:
    """
    Derive the canonical pump pool PDA.
    Seeds: ["pool", index=0, authority, mint, WSOL]
    """
    index = 0
    authority = get_pump_pool_authority_pda(mint)
    seeds = [
        POOL_SEED,
        struct.pack("<H", index),  # u16 index = 0
        bytes(authority),
        bytes(mint),
        bytes(WSOL_TOKEN_ACCOUNT),
    ]
    (pda, _) = Pubkey.find_program_address(seeds, PUMPSWAP_PROGRAM_ID)
    return pda


def get_user_volume_accumulator_pda(user: Pubkey) -> Pubkey:
    """
    Derive the user volume accumulator PDA for a given user.
    Seeds: ["user_volume_accumulator", user]
    """
    seeds = [USER_VOLUME_ACCUMULATOR_SEED, bytes(user)]
    (pda, _) = Pubkey.find_program_address(seeds, PUMPSWAP_PROGRAM_ID)
    return pda


def get_user_volume_accumulator_wsol_ata(user: Pubkey) -> Pubkey:
    """
    Get the WSOL ATA of UserVolumeAccumulator for PumpSwap AMM.
    """
    accumulator = get_user_volume_accumulator_pda(user)
    return get_associated_token_address(accumulator, WSOL_TOKEN_ACCOUNT, TOKEN_PROGRAM)


def get_user_volume_accumulator_quote_ata(
    user: Pubkey,
    quote_mint: Pubkey,
    quote_token_program: Pubkey,
) -> Pubkey:
    """
    Get the quote-mint ATA of UserVolumeAccumulator (used for sell cashback).
    """
    accumulator = get_user_volume_accumulator_pda(user)
    return get_associated_token_address(accumulator, quote_mint, quote_token_program)


def coin_creator_vault_authority(coin_creator: Pubkey) -> Pubkey:
    """
    Derive the coin creator vault authority PDA.
    Seeds: ["creator_vault", coin_creator]
    """
    seeds = [b"creator_vault", bytes(coin_creator)]
    (pda, _) = Pubkey.find_program_address(seeds, PUMPSWAP_PROGRAM_ID)
    return pda


def coin_creator_vault_ata(coin_creator: Pubkey, quote_mint: Pubkey) -> Pubkey:
    """
    Get the coin creator vault ATA.
    """
    authority = coin_creator_vault_authority(coin_creator)
    return get_associated_token_address(authority, quote_mint, TOKEN_PROGRAM)


def fee_recipient_ata(fee_recipient: Pubkey, quote_mint: Pubkey) -> Pubkey:
    """
    Get the fee recipient ATA.
    """
    return get_associated_token_address(fee_recipient, quote_mint, TOKEN_PROGRAM)


def get_mayhem_fee_recipient_random() -> Tuple[Pubkey, AccountMeta]:
    """
    Get a random Mayhem fee recipient and its AccountMeta.
    """
    recipient = random.choice(MAYHEM_FEE_RECIPIENTS)
    meta = AccountMeta(recipient, False, False)
    return (recipient, meta)


# ============================================
# PumpSwap Parameters Dataclass
# ============================================

@dataclass
class PumpSwapParams:
    """Parameters for PumpSwap protocol trading."""
    pool: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    base_mint: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    quote_mint: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    pool_base_token_account: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    pool_quote_token_account: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    pool_base_token_reserves: int = 0
    pool_quote_token_reserves: int = 0
    coin_creator_vault_ata: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    coin_creator_vault_authority: Pubkey = Pubkey.from_string("11111111111111111111111111111111")
    base_token_program: Pubkey = TOKEN_PROGRAM
    quote_token_program: Pubkey = TOKEN_PROGRAM
    is_mayhem_mode: bool = False
    is_cashback_coin: bool = False

    @property
    def is_wsol(self) -> bool:
        """Check if the pool contains WSOL."""
        return (
            (self.base_mint == WSOL_TOKEN_ACCOUNT and self.quote_mint != USDC_TOKEN_ACCOUNT)
            or (self.quote_mint == WSOL_TOKEN_ACCOUNT and self.base_mint != USDC_TOKEN_ACCOUNT)
        )

    @property
    def is_usdc(self) -> bool:
        """Check if the pool contains USDC."""
        return (
            (self.base_mint == USDC_TOKEN_ACCOUNT and self.quote_mint != WSOL_TOKEN_ACCOUNT)
            or (self.quote_mint == USDC_TOKEN_ACCOUNT and self.base_mint != WSOL_TOKEN_ACCOUNT)
        )


# ============================================
# PumpSwap Calculation Functions
# ============================================

def buy_quote_input_internal(
    quote_amount: int,
    slippage_bps: int,
    pool_base_reserves: int,
    pool_quote_reserves: int,
    creator: Pubkey,
) -> Tuple[int, int]:
    """
    Calculate buy amounts for PumpSwap when quote is input.
    Returns (base_amount_out, max_quote_amount_in).
    """
    default_pubkey = Pubkey.from_string("11111111111111111111111111111111")
    is_non_zero_creator = creator != default_pubkey

    # Apply LP fee
    quote_amount_after_fee = quote_amount - (quote_amount * LP_FEE_BASIS_POINTS) // 10000

    # Apply protocol fee
    quote_amount_after_fee = quote_amount_after_fee - (quote_amount * PROTOCOL_FEE_BASIS_POINTS) // 10000

    # Apply creator fee if applicable
    if is_non_zero_creator:
        quote_amount_after_fee = quote_amount_after_fee - (quote_amount * COIN_CREATOR_FEE_BASIS_POINTS) // 10000

    # Calculate output using AMM formula
    # out = (quote_in * base_reserves) / (quote_reserves + quote_in)
    numerator = quote_amount_after_fee * pool_base_reserves
    denominator = pool_quote_reserves + quote_amount_after_fee
    base_amount_out = numerator // denominator

    # Calculate max quote with slippage
    max_quote = int(quote_amount * (10000 + slippage_bps) / 10000)

    return (base_amount_out, max_quote)


def sell_base_input_internal(
    base_amount: int,
    slippage_bps: int,
    pool_base_reserves: int,
    pool_quote_reserves: int,
    creator: Pubkey,
) -> Tuple[int, int]:
    """
    Calculate sell amounts for PumpSwap when base is input.
    Returns (min_quote_amount_out, base_amount_in).
    """
    default_pubkey = Pubkey.from_string("11111111111111111111111111111111")
    is_non_zero_creator = creator != default_pubkey

    # Apply LP fee
    base_amount_after_fee = base_amount - (base_amount * LP_FEE_BASIS_POINTS) // 10000

    # Apply protocol fee
    base_amount_after_fee = base_amount_after_fee - (base_amount * PROTOCOL_FEE_BASIS_POINTS) // 10000

    # Apply creator fee if applicable
    if is_non_zero_creator:
        base_amount_after_fee = base_amount_after_fee - (base_amount * COIN_CREATOR_FEE_BASIS_POINTS) // 10000

    # Calculate output using AMM formula
    # out = (base_in * quote_reserves) / (base_reserves + base_in)
    numerator = base_amount_after_fee * pool_quote_reserves
    denominator = pool_base_reserves + base_amount_after_fee
    quote_amount_out = numerator // denominator

    # Apply slippage
    min_quote = calculate_with_slippage_sell(quote_amount_out, slippage_bps)

    return (min_quote, base_amount)


# ============================================
# Build Buy Instructions
# ============================================

def build_buy_instructions(
    payer: Pubkey,
    output_mint: Pubkey,
    input_amount: int,
    params: PumpSwapParams,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_input_ata: bool = True,
    create_output_ata: bool = True,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
    use_exact_quote_amount: bool = True,
) -> List[Instruction]:
    """
    Build PumpSwap buy instructions.

    Args:
        payer: The wallet paying for the swap
        output_mint: The token mint to buy (base_mint)
        input_amount: Amount of SOL/USDC to spend
        params: PumpSwap protocol parameters
        slippage_bps: Slippage tolerance in basis points
        create_input_ata: Whether to create WSOL ATA if needed
        create_output_ata: Whether to create output token ATA if needed
        close_input_ata: Whether to close WSOL ATA after swap
        fixed_output_amount: If set, use this as exact output amount
        use_exact_quote_amount: If True, use buy_exact_quote_in instruction

    Returns:
        List of instructions for the buy operation
    """
    if input_amount == 0:
        raise ValueError("Amount cannot be zero")

    instructions = []

    # Validate pool contains WSOL or USDC
    if not params.is_wsol and not params.is_usdc:
        raise ValueError("Pool must contain WSOL or USDC")

    # Determine if quote is WSOL/USDC
    quote_is_wsol_or_usdc = (
        params.quote_mint == WSOL_TOKEN_ACCOUNT or params.quote_mint == USDC_TOKEN_ACCOUNT
    )

    # Get creator
    creator = Pubkey.from_string("11111111111111111111111111111111")
    if params.coin_creator_vault_authority != DEFAULT_COIN_CREATOR_VAULT_AUTHORITY:
        creator = params.coin_creator_vault_authority

    # Calculate amounts
    if quote_is_wsol_or_usdc:
        result = buy_quote_input_internal(
            input_amount,
            slippage_bps,
            params.pool_base_token_reserves,
            params.pool_quote_token_reserves,
            creator,
        )
        token_amount, sol_amount = result[0], result[1]
    else:
        result = sell_base_input_internal(
            input_amount,
            slippage_bps,
            params.pool_base_token_reserves,
            params.pool_quote_token_reserves,
            creator,
        )
        token_amount, sol_amount = result[1], result[0]

    if fixed_output_amount is not None:
        token_amount = fixed_output_amount

    # Get user token accounts
    user_base_token_account = get_associated_token_address(
        payer, params.base_mint, params.base_token_program
    )
    user_quote_token_account = get_associated_token_address(
        payer, params.quote_mint, params.quote_token_program
    )

    # Determine fee recipient
    if params.is_mayhem_mode:
        fee_recipient, fee_recipient_meta = get_mayhem_fee_recipient_random()
    else:
        fee_recipient = FEE_RECIPIENT
        fee_recipient_meta = AccountMeta(FEE_RECIPIENT, False, False)

    # Get fee recipient ATA
    if params.is_mayhem_mode:
        fee_recipient_ata_addr = fee_recipient_ata(fee_recipient, WSOL_TOKEN_ACCOUNT)
    else:
        fee_recipient_ata_addr = fee_recipient_ata(fee_recipient, params.quote_mint)

    # Handle WSOL if needed
    if create_input_ata:
        wrap_amount = input_amount if quote_is_wsol_or_usdc and use_exact_quote_amount else sol_amount
        if params.is_wsol:
            instructions.extend(handle_wsol(payer, wrap_amount))

    # Create output ATA if needed
    if create_output_ata:
        output_mint_to_create = params.base_mint if quote_is_wsol_or_usdc else params.quote_mint
        output_program = params.base_token_program if quote_is_wsol_or_usdc else params.quote_token_program
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, output_mint_to_create, output_program
            )
        )

    # Build accounts list
    accounts = [
        AccountMeta(params.pool, False, True),  # pool_id (writable)
        AccountMeta(payer, True, True),  # user (signer, writable)
        AccountMeta(GLOBAL_ACCOUNT, False, False),  # global (readonly)
        AccountMeta(params.base_mint, False, False),  # base_mint (readonly)
        AccountMeta(params.quote_mint, False, False),  # quote_mint (readonly)
        AccountMeta(user_base_token_account, False, True),  # user_base_token_account (writable)
        AccountMeta(user_quote_token_account, False, True),  # user_quote_token_account (writable)
        AccountMeta(params.pool_base_token_account, False, True),  # pool_base_token_account (writable)
        AccountMeta(params.pool_quote_token_account, False, True),  # pool_quote_token_account (writable)
        fee_recipient_meta,  # fee_recipient (readonly)
        AccountMeta(fee_recipient_ata_addr, False, True),  # fee_recipient_ata (writable)
        AccountMeta(params.base_token_program, False, False),  # base_token_program (readonly)
        AccountMeta(params.quote_token_program, False, False),  # quote_token_program (readonly)
        AccountMeta(SYSTEM_PROGRAM, False, False),  # system_program (readonly)
        AccountMeta(ASSOCIATED_TOKEN_PROGRAM, False, False),  # associated_token_program (readonly)
        AccountMeta(EVENT_AUTHORITY, False, False),  # event_authority (readonly)
        AccountMeta(PUMPSWAP_PROGRAM_ID, False, False),  # program (readonly)
        AccountMeta(params.coin_creator_vault_ata, False, True),  # coin_creator_vault_ata (writable)
        AccountMeta(params.coin_creator_vault_authority, False, False),  # coin_creator_vault_authority (readonly)
    ]

    # Add volume accumulator accounts for WSOL/USDC pools
    if quote_is_wsol_or_usdc:
        accounts.append(AccountMeta(GLOBAL_VOLUME_ACCUMULATOR, False, True))
        uva = get_user_volume_accumulator_pda(payer)
        accounts.append(AccountMeta(uva, False, True))

    # Add fee accounts
    accounts.append(AccountMeta(FEE_CONFIG, False, False))
    accounts.append(AccountMeta(FEE_PROGRAM, False, False))

    # Add cashback WSOL ATA
    if params.is_cashback_coin:
        wsol_ata = get_user_volume_accumulator_wsol_ata(payer)
        accounts.append(AccountMeta(wsol_ata, False, True))

    # Add pool v2 PDA
    pool_v2 = get_pool_v2_pda(params.base_mint)
    accounts.append(AccountMeta(pool_v2, False, False))

    # Build instruction data
    track_volume = bytes([1, 1]) if params.is_cashback_coin else bytes([1, 0])

    if quote_is_wsol_or_usdc:
        if use_exact_quote_amount:
            min_base_out = calculate_with_slippage_sell(token_amount, slippage_bps)
            data = BUY_EXACT_QUOTE_IN_DISCRIMINATOR + struct.pack("<QQ", input_amount, min_base_out) + track_volume
        else:
            data = BUY_DISCRIMINATOR + struct.pack("<QQ", token_amount, sol_amount) + track_volume
    else:
        data = SELL_DISCRIMINATOR + struct.pack("<QQ", sol_amount, token_amount)

    instructions.append(Instruction(PUMPSWAP_PROGRAM_ID, data, accounts))

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
    params: PumpSwapParams,
    slippage_bps: int = DEFAULT_SLIPPAGE,
    create_output_ata: bool = True,
    close_output_ata: bool = False,
    close_input_ata: bool = False,
    fixed_output_amount: Optional[int] = None,
) -> List[Instruction]:
    """
    Build PumpSwap sell instructions.

    Args:
        payer: The wallet paying for the swap
        input_mint: The token mint to sell (base_mint)
        input_amount: Amount of tokens to sell
        params: PumpSwap protocol parameters
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

    # Determine if quote is WSOL/USDC
    quote_is_wsol_or_usdc = (
        params.quote_mint == WSOL_TOKEN_ACCOUNT or params.quote_mint == USDC_TOKEN_ACCOUNT
    )

    # Get creator
    creator = Pubkey.from_string("11111111111111111111111111111111")
    if params.coin_creator_vault_authority != DEFAULT_COIN_CREATOR_VAULT_AUTHORITY:
        creator = params.coin_creator_vault_authority

    # Calculate amounts
    if quote_is_wsol_or_usdc:
        result = sell_base_input_internal(
            input_amount,
            slippage_bps,
            params.pool_base_token_reserves,
            params.pool_quote_token_reserves,
            creator,
        )
        token_amount, sol_amount = input_amount, result[0]
    else:
        result = buy_quote_input_internal(
            input_amount,
            slippage_bps,
            params.pool_base_token_reserves,
            params.pool_quote_token_reserves,
            creator,
        )
        token_amount, sol_amount = result[1], result[0]

    if fixed_output_amount is not None:
        sol_amount = fixed_output_amount

    # Get user token accounts
    user_base_token_account = get_associated_token_address(
        payer, params.base_mint, params.base_token_program
    )
    user_quote_token_account = get_associated_token_address(
        payer, params.quote_mint, params.quote_token_program
    )

    # Determine fee recipient
    if params.is_mayhem_mode:
        fee_recipient, fee_recipient_meta = get_mayhem_fee_recipient_random()
    else:
        fee_recipient = FEE_RECIPIENT
        fee_recipient_meta = AccountMeta(FEE_RECIPIENT, False, False)

    # Get fee recipient ATA
    if params.is_mayhem_mode:
        fee_recipient_ata_addr = fee_recipient_ata(fee_recipient, WSOL_TOKEN_ACCOUNT)
    else:
        fee_recipient_ata_addr = fee_recipient_ata(fee_recipient, params.quote_mint)

    # Create WSOL ATA if needed
    if create_output_ata and params.is_wsol:
        instructions.append(
            create_associated_token_account_idempotent_instruction(
                payer, payer, WSOL_TOKEN_ACCOUNT, TOKEN_PROGRAM
            )
        )

    # Build accounts list
    accounts = [
        AccountMeta(params.pool, False, True),  # pool_id (writable)
        AccountMeta(payer, True, True),  # user (signer, writable)
        AccountMeta(GLOBAL_ACCOUNT, False, False),  # global (readonly)
        AccountMeta(params.base_mint, False, False),  # base_mint (readonly)
        AccountMeta(params.quote_mint, False, False),  # quote_mint (readonly)
        AccountMeta(user_base_token_account, False, True),  # user_base_token_account (writable)
        AccountMeta(user_quote_token_account, False, True),  # user_quote_token_account (writable)
        AccountMeta(params.pool_base_token_account, False, True),  # pool_base_token_account (writable)
        AccountMeta(params.pool_quote_token_account, False, True),  # pool_quote_token_account (writable)
        fee_recipient_meta,  # fee_recipient (readonly)
        AccountMeta(fee_recipient_ata_addr, False, True),  # fee_recipient_ata (writable)
        AccountMeta(params.base_token_program, False, False),  # base_token_program (readonly)
        AccountMeta(params.quote_token_program, False, False),  # quote_token_program (readonly)
        AccountMeta(SYSTEM_PROGRAM, False, False),  # system_program (readonly)
        AccountMeta(ASSOCIATED_TOKEN_PROGRAM, False, False),  # associated_token_program (readonly)
        AccountMeta(EVENT_AUTHORITY, False, False),  # event_authority (readonly)
        AccountMeta(PUMPSWAP_PROGRAM_ID, False, False),  # program (readonly)
        AccountMeta(params.coin_creator_vault_ata, False, True),  # coin_creator_vault_ata (writable)
        AccountMeta(params.coin_creator_vault_authority, False, False),  # coin_creator_vault_authority (readonly)
    ]

    # Add volume accumulator accounts for non-WSOL/USDC pools
    if not quote_is_wsol_or_usdc:
        accounts.append(AccountMeta(GLOBAL_VOLUME_ACCUMULATOR, False, True))
        uva = get_user_volume_accumulator_pda(payer)
        accounts.append(AccountMeta(uva, False, True))

    # Add fee accounts
    accounts.append(AccountMeta(FEE_CONFIG, False, False))
    accounts.append(AccountMeta(FEE_PROGRAM, False, False))

    # Add cashback accounts for sell
    if params.is_cashback_coin:
        quote_ata = get_user_volume_accumulator_quote_ata(
            payer, params.quote_mint, params.quote_token_program
        )
        accumulator = get_user_volume_accumulator_pda(payer)
        accounts.append(AccountMeta(quote_ata, False, True))
        accounts.append(AccountMeta(accumulator, False, True))

    # Add pool v2 PDA
    pool_v2 = get_pool_v2_pda(params.base_mint)
    accounts.append(AccountMeta(pool_v2, False, False))

    # Build instruction data
    if quote_is_wsol_or_usdc:
        data = SELL_DISCRIMINATOR + struct.pack("<QQ", token_amount, sol_amount)
    else:
        data = BUY_DISCRIMINATOR + struct.pack("<QQ", sol_amount, token_amount)

    instructions.append(Instruction(PUMPSWAP_PROGRAM_ID, data, accounts))

    # Close WSOL ATA if requested
    if close_output_ata and params.is_wsol:
        instructions.extend(close_wsol(payer))

    # Close token ATA if requested
    if close_input_ata:
        token_program = params.base_token_program if quote_is_wsol_or_usdc else params.quote_token_program
        token_account = user_base_token_account if quote_is_wsol_or_usdc else user_quote_token_account
        instructions.append(
            close_token_account_instruction(
                token_program,
                token_account,
                payer,
                payer,
            )
        )

    return instructions


# ============================================
# Claim Cashback Instruction
# ============================================

def claim_cashback_pumpswap_instruction(
    payer: Pubkey,
    quote_mint: Pubkey,
    quote_token_program: Pubkey,
) -> Instruction:
    """
    Build instruction to claim cashback for PumpSwap AMM.
    Transfers WSOL from UserVolumeAccumulator's WSOL ATA to user's WSOL ATA.
    """
    user_volume_accumulator = get_user_volume_accumulator_pda(payer)
    user_volume_accumulator_wsol_ata = get_user_volume_accumulator_wsol_ata(payer)
    user_wsol_ata = get_associated_token_address(payer, quote_mint, quote_token_program)

    accounts = [
        AccountMeta(payer, True, True),  # user (signer, writable)
        AccountMeta(user_volume_accumulator, False, True),  # user_volume_accumulator (writable)
        AccountMeta(quote_mint, False, False),  # quote_mint (readonly)
        AccountMeta(quote_token_program, False, False),  # quote_token_program (readonly)
        AccountMeta(user_volume_accumulator_wsol_ata, False, True),  # user_volume_accumulator_wsol_token_account (writable)
        AccountMeta(user_wsol_ata, False, True),  # user_wsol_token_account (writable)
        AccountMeta(SYSTEM_PROGRAM, False, False),  # system_program
        AccountMeta(EVENT_AUTHORITY, False, False),  # event_authority
        AccountMeta(PUMPSWAP_PROGRAM_ID, False, False),  # program
    ]

    return Instruction(PUMPSWAP_PROGRAM_ID, CLAIM_CASHBACK_DISCRIMINATOR, accounts)


# ============================================
# Exports
# ============================================

__all__ = [
    # Program IDs and Constants
    "PUMPSWAP_PROGRAM_ID",
    "FEE_RECIPIENT",
    "GLOBAL_ACCOUNT",
    "EVENT_AUTHORITY",
    "ASSOCIATED_TOKEN_PROGRAM",
    "PROTOCOL_FEE_RECIPIENT",
    "PUMP_PROGRAM_ID",
    "FEE_PROGRAM",
    "GLOBAL_VOLUME_ACCUMULATOR",
    "FEE_CONFIG",
    "DEFAULT_COIN_CREATOR_VAULT_AUTHORITY",
    "MAYHEM_FEE_RECIPIENTS",
    "LP_FEE_BASIS_POINTS",
    "PROTOCOL_FEE_BASIS_POINTS",
    "COIN_CREATOR_FEE_BASIS_POINTS",
    # Discriminators
    "BUY_DISCRIMINATOR",
    "BUY_EXACT_QUOTE_IN_DISCRIMINATOR",
    "SELL_DISCRIMINATOR",
    "CLAIM_CASHBACK_DISCRIMINATOR",
    # PDA Functions
    "get_pool_v2_pda",
    "get_pump_pool_authority_pda",
    "get_canonical_pool_pda",
    "get_user_volume_accumulator_pda",
    "get_user_volume_accumulator_wsol_ata",
    "get_user_volume_accumulator_quote_ata",
    "coin_creator_vault_authority",
    "coin_creator_vault_ata",
    "fee_recipient_ata",
    "get_mayhem_fee_recipient_random",
    # Params
    "PumpSwapParams",
    # Calculation Functions
    "buy_quote_input_internal",
    "sell_base_input_internal",
    # Instruction Builders
    "build_buy_instructions",
    "build_sell_instructions",
    "claim_cashback_pumpswap_instruction",
]
