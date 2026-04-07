/**
 * PumpFun Protocol Instruction Builder
 *
 * Production-grade instruction builder for PumpFun bonding curve protocol.
 * Supports buy, sell, and cashback claim operations.
 */

import {
  PublicKey,
  Keypair,
  AccountMeta,
  Instruction,
  SYSVAR_RENT_PUBKEY,
  SystemProgram,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  createCloseAccountInstruction,
} from "@solana/spl-token";

// ============================================
// Program IDs and Constants
// ============================================

/** PumpFun program ID */
export const PUMPFUN_PROGRAM_ID = new PublicKey(
  "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
);

/** MPL Token Metadata program ID */
export const MPL_TOKEN_METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);

/** Event Authority for PumpFun */
export const EVENT_AUTHORITY = new PublicKey(
  "Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1"
);

/** Fee Program */
export const FEE_PROGRAM = new PublicKey(
  "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ"
);

/** Global Volume Accumulator */
export const GLOBAL_VOLUME_ACCUMULATOR = new PublicKey(
  "Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y"
);

/** Fee Config */
export const FEE_CONFIG = new PublicKey(
  "8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt"
);

/** Global Account */
export const GLOBAL_ACCOUNT = new PublicKey(
  "4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf"
);

/** Authority */
export const AUTHORITY = new PublicKey(
  "FFWtrEQ4B4PKQoVuHYzZq8FabGkVatYzDpEVHsK5rrhF"
);

/** Withdraw Authority */
export const WITHDRAW_AUTHORITY = new PublicKey(
  "39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg"
);

/** Fee Recipient */
export const FEE_RECIPIENT = new PublicKey(
  "62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV"
);

/** Mayhem Fee Recipients */
export const MAYHEM_FEE_RECIPIENTS: PublicKey[] = [
  new PublicKey("GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS"),
  new PublicKey("4budycTjhs9fD6xw62VBducVTNgMgJJ5BgtKq7mAZwn6"),
  new PublicKey("8SBKzEQU4nLSzcwF4a74F2iaUDQyTfjGndn6qUWBnrpR"),
  new PublicKey("4UQeTP1T39KZ9Sfxzo3WR5skgsaP6NZa87BAkuazLEKH"),
  new PublicKey("8sNeir4QsLsJdYpc9RZacohhK1Y5FLU3nC5LXgYB4aa6"),
  new PublicKey("Fh9HmeLNUMVCvejxCtCL2DbYaRyBFVJ5xrWkLnMH6fdk"),
  new PublicKey("463MEnMeGyJekNZFQSTUABBEbLnvMTALbT6ZmsxAbAdq"),
  new PublicKey("6AUH3WEHucYZyC61hqpqYUWVto5qA5hjHuNQ32GNnNxA"),
];

// ============================================
// Discriminators
// ============================================

/** Buy instruction discriminator */
export const BUY_DISCRIMINATOR: Buffer = Buffer.from([
  102, 6, 61, 18, 1, 218, 235, 234,
]);

/** Buy exact SOL in discriminator */
export const BUY_EXACT_SOL_IN_DISCRIMINATOR: Buffer = Buffer.from([
  56, 252, 116, 8, 158, 223, 205, 95,
]);

/** Sell instruction discriminator */
export const SELL_DISCRIMINATOR: Buffer = Buffer.from([
  51, 230, 133, 164, 1, 127, 131, 173,
]);

/** Claim cashback discriminator */
export const CLAIM_CASHBACK_DISCRIMINATOR: Buffer = Buffer.from([
  37, 58, 35, 126, 190, 53, 228, 197,
]);

// ============================================
// Seeds
// ============================================

export const BONDING_CURVE_SEED = Buffer.from("bonding-curve");
export const BONDING_CURVE_V2_SEED = Buffer.from("bonding-curve-v2");
export const CREATOR_VAULT_SEED = Buffer.from("creator-vault");
export const USER_VOLUME_ACCUMULATOR_SEED = Buffer.from("user_volume_accumulator");

// ============================================
// PDA Derivation Functions
// ============================================

/**
 * Derive the bonding curve PDA for a given mint
 */
export function getBondingCurvePda(mint: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [BONDING_CURVE_SEED, mint.toBuffer()],
    PUMPFUN_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive the bonding curve v2 PDA for a given mint
 */
export function getBondingCurveV2Pda(mint: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [BONDING_CURVE_V2_SEED, mint.toBuffer()],
    PUMPFUN_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive the creator vault PDA for a given creator
 */
export function getCreatorVaultPda(creator: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [CREATOR_VAULT_SEED, creator.toBuffer()],
    PUMPFUN_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive the user volume accumulator PDA for a given user
 */
export function getUserVolumeAccumulatorPda(user: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [USER_VOLUME_ACCUMULATOR_SEED, user.toBuffer()],
    PUMPFUN_PROGRAM_ID
  );
  return pda;
}

/**
 * Get a random Mayhem fee recipient
 */
export function getRandomMayhemFeeRecipient(): PublicKey {
  const index = Math.floor(Math.random() * MAYHEM_FEE_RECIPIENTS.length);
  return MAYHEM_FEE_RECIPIENTS[index];
}

// ============================================
// Helper Functions
// ============================================

/**
 * Calculate buy amount with slippage protection
 */
export function calculateWithSlippageBuy(amount: bigint, basisPoints: bigint): bigint {
  const maxBps = 9999n;
  const bps = basisPoints > maxBps ? maxBps : basisPoints;
  return amount + (amount * bps) / 10000n;
}

/**
 * Calculate sell amount with slippage protection
 */
export function calculateWithSlippageSell(amount: bigint, basisPoints: bigint): bigint {
  if (amount <= basisPoints / 10000n) {
    return 1n;
  }
  return amount - (amount * basisPoints) / 10000n;
}

// ============================================
// Types
// ============================================

export interface BondingCurve {
  account: PublicKey;
  virtualTokenReserves: bigint;
  virtualSolReserves: bigint;
  realTokenReserves: bigint;
  isMayhemMode: boolean;
  isCashbackCoin: boolean;
}

export interface PumpFunParams {
  bondingCurve: BondingCurve;
  creatorVault: PublicKey;
  tokenProgram: PublicKey;
  associatedBondingCurve?: PublicKey;
  closeTokenAccountWhenSell?: boolean;
}

export interface BuildBuyInstructionsParams {
  payer: Keypair | PublicKey;
  outputMint: PublicKey;
  inputAmount: bigint;
  slippageBasisPoints?: bigint;
  fixedOutputAmount?: bigint;
  createOutputMintAta?: boolean;
  protocolParams: PumpFunParams;
  useExactSolAmount?: boolean;
}

export interface BuildSellInstructionsParams {
  payer: Keypair | PublicKey;
  inputMint: PublicKey;
  inputAmount: bigint;
  slippageBasisPoints?: bigint;
  fixedOutputAmount?: bigint;
  closeInputMintAta?: boolean;
  protocolParams: PumpFunParams;
}

// ============================================
// Instruction Builders
// ============================================

/**
 * Build buy instructions for PumpFun protocol
 */
export function buildBuyInstructions(
  params: BuildBuyInstructionsParams
): Instruction[] {
  const {
    payer,
    outputMint,
    inputAmount,
    slippageBasisPoints = 1000n,
    fixedOutputAmount,
    createOutputMintAta = true,
    protocolParams,
    useExactSolAmount = true,
  } = params;

  if (inputAmount === 0n) {
    throw new Error("Amount cannot be zero");
  }

  const payerPubkey = payer instanceof Keypair ? payer.publicKey : payer;
  const instructions: Instruction[] = [];

  const { bondingCurve, creatorVault, tokenProgram, associatedBondingCurve } = protocolParams;

  // Derive bonding curve address
  const bondingCurveAddr =
    bondingCurve.account.equals(PublicKey.default) || bondingCurve.account === undefined
      ? getBondingCurvePda(outputMint)
      : bondingCurve.account;

  // Get token program
  const tokenProgramId = tokenProgram || TOKEN_PROGRAM_ID;

  // Derive associated bonding curve
  const associatedBondingCurveAddr =
    associatedBondingCurve && !associatedBondingCurve.equals(PublicKey.default)
      ? associatedBondingCurve
      : getAssociatedTokenAddressSync(outputMint, bondingCurveAddr, true, tokenProgramId);

  // Derive user token account
  const userTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    tokenProgramId
  );

  // Derive user volume accumulator
  const userVolumeAccumulator = getUserVolumeAccumulatorPda(payerPubkey);

  // Create ATA if needed
  if (createOutputMintAta) {
    instructions.push(
      createAssociatedTokenAccountInstruction(
        payerPubkey,
        userTokenAccount,
        payerPubkey,
        outputMint,
        tokenProgramId
      )
    );
  }

  // Determine fee recipient
  const feeRecipient = bondingCurve.isMayhemMode
    ? getRandomMayhemFeeRecipient()
    : FEE_RECIPIENT;

  // Derive bonding curve v2
  const bondingCurveV2 = getBondingCurveV2Pda(outputMint);

  // Track volume for cashback coins
  const trackVolume = bondingCurve.isCashbackCoin
    ? Buffer.from([1, 1])
    : Buffer.from([1, 0]);

  // Build instruction data
  let data: Buffer;
  if (useExactSolAmount) {
    // buy_exact_sol_in(spendable_sol_in: u64, min_tokens_out: u64, track_volume)
    const minTokensOut = fixedOutputAmount
      ? fixedOutputAmount
      : calculateWithSlippageSell(inputAmount, slippageBasisPoints);
    data = Buffer.alloc(26);
    BUY_EXACT_SOL_IN_DISCRIMINATOR.copy(data, 0);
    data.writeBigUInt64LE(inputAmount, 8);
    data.writeBigUInt64LE(minTokensOut, 16);
    trackVolume.copy(data, 24);
  } else {
    // buy(token_amount: u64, max_sol_cost: u64, track_volume)
    const maxSolCost = calculateWithSlippageBuy(inputAmount, slippageBasisPoints);
    data = Buffer.alloc(26);
    BUY_DISCRIMINATOR.copy(data, 0);
    data.writeBigUInt64LE(inputAmount, 8);
    data.writeBigUInt64LE(maxSolCost, 16);
    trackVolume.copy(data, 24);
  }

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: GLOBAL_ACCOUNT, isSigner: false, isWritable: false },
    { pubkey: feeRecipient, isSigner: false, isWritable: true },
    { pubkey: outputMint, isSigner: false, isWritable: false },
    { pubkey: bondingCurveAddr, isSigner: false, isWritable: true },
    { pubkey: associatedBondingCurveAddr, isSigner: false, isWritable: true },
    { pubkey: userTokenAccount, isSigner: false, isWritable: true },
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
    { pubkey: creatorVault, isSigner: false, isWritable: true },
    { pubkey: EVENT_AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: PUMPFUN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: GLOBAL_VOLUME_ACCUMULATOR, isSigner: false, isWritable: true },
    { pubkey: userVolumeAccumulator, isSigner: false, isWritable: true },
    { pubkey: FEE_CONFIG, isSigner: false, isWritable: false },
    { pubkey: FEE_PROGRAM, isSigner: false, isWritable: false },
    { pubkey: bondingCurveV2, isSigner: false, isWritable: false },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: PUMPFUN_PROGRAM_ID,
      data,
    })
  );

  return instructions;
}

/**
 * Build sell instructions for PumpFun protocol
 */
export function buildSellInstructions(
  params: BuildSellInstructionsParams
): Instruction[] {
  const {
    payer,
    inputMint,
    inputAmount,
    slippageBasisPoints = 1000n,
    fixedOutputAmount,
    closeInputMintAta = false,
    protocolParams,
  } = params;

  if (inputAmount === 0n) {
    throw new Error("Amount cannot be zero");
  }

  const payerPubkey = payer instanceof Keypair ? payer.publicKey : payer;
  const instructions: Instruction[] = [];

  const { bondingCurve, creatorVault, tokenProgram, associatedBondingCurve, closeTokenAccountWhenSell } = protocolParams;

  // Derive bonding curve address
  const bondingCurveAddr =
    bondingCurve.account.equals(PublicKey.default) || bondingCurve.account === undefined
      ? getBondingCurvePda(inputMint)
      : bondingCurve.account;

  // Get token program
  const tokenProgramId = tokenProgram || TOKEN_PROGRAM_ID;

  // Derive associated bonding curve
  const associatedBondingCurveAddr =
    associatedBondingCurve && !associatedBondingCurve.equals(PublicKey.default)
      ? associatedBondingCurve
      : getAssociatedTokenAddressSync(inputMint, bondingCurveAddr, true, tokenProgramId);

  // Derive user token account
  const userTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    tokenProgramId
  );

  // Determine fee recipient
  const feeRecipient = bondingCurve.isMayhemMode
    ? getRandomMayhemFeeRecipient()
    : FEE_RECIPIENT;

  // Derive bonding curve v2
  const bondingCurveV2 = getBondingCurveV2Pda(inputMint);

  // Build instruction data (sell: token_amount, min_sol_output)
  const minSolOutput = fixedOutputAmount
    ? fixedOutputAmount
    : calculateWithSlippageSell(inputAmount, slippageBasisPoints);
  const data = Buffer.alloc(24);
  SELL_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(minSolOutput, 16);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: GLOBAL_ACCOUNT, isSigner: false, isWritable: false },
    { pubkey: feeRecipient, isSigner: false, isWritable: true },
    { pubkey: inputMint, isSigner: false, isWritable: false },
    { pubkey: bondingCurveAddr, isSigner: false, isWritable: true },
    { pubkey: associatedBondingCurveAddr, isSigner: false, isWritable: true },
    { pubkey: userTokenAccount, isSigner: false, isWritable: true },
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: creatorVault, isSigner: false, isWritable: true },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
    { pubkey: EVENT_AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: PUMPFUN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: FEE_CONFIG, isSigner: false, isWritable: false },
    { pubkey: FEE_PROGRAM, isSigner: false, isWritable: false },
  ];

  // Add user volume accumulator for cashback coins
  if (bondingCurve.isCashbackCoin) {
    const userVolumeAccumulator = getUserVolumeAccumulatorPda(payerPubkey);
    accounts.push({ pubkey: userVolumeAccumulator, isSigner: false, isWritable: true });
  }

  // Add bonding curve v2
  accounts.push({ pubkey: bondingCurveV2, isSigner: false, isWritable: false });

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: PUMPFUN_PROGRAM_ID,
      data,
    })
  );

  // Close token account if requested
  if (closeInputMintAta || closeTokenAccountWhenSell) {
    instructions.push(
      createCloseAccountInstruction(
        userTokenAccount,
        payerPubkey,
        payerPubkey,
        [],
        tokenProgramId
      )
    );
  }

  return instructions;
}

/**
 * Build claim cashback instruction for PumpFun
 */
export function buildClaimCashbackInstruction(payer: PublicKey): Instruction {
  const userVolumeAccumulator = getUserVolumeAccumulatorPda(payer);

  const accounts: AccountMeta[] = [
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey: userVolumeAccumulator, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: EVENT_AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: PUMPFUN_PROGRAM_ID, isSigner: false, isWritable: false },
  ];

  return new Instruction({
    keys: accounts,
    programId: PUMPFUN_PROGRAM_ID,
    data: CLAIM_CASHBACK_DISCRIMINATOR,
  });
}
