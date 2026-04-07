/**
 * Bonk Protocol Instruction Builder
 *
 * Production-grade instruction builder for Bonk AMM protocol.
 * Supports buy and sell operations with WSOL and USD1 pools.
 */

import {
  PublicKey,
  Keypair,
  AccountMeta,
  Instruction,
  SystemProgram,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  TOKEN_PROGRAM_ID,
  createCloseAccountInstruction,
  NATIVE_MINT,
  createSyncNativeInstruction,
  createAccount,
  getAccount,
  getMint,
  Account as TokenAccount,
} from "@solana/spl-token";

// ============================================
// Program IDs and Constants
// ============================================

/** Bonk program ID */
export const BONK_PROGRAM_ID = new PublicKey(
  "LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj"
);

/** Authority */
export const AUTHORITY = new PublicKey(
  "WLhv2UAZm6z4KyaaELi5pjdbJh6RESMva1Rnn8pJVVh"
);

/** Global Config */
export const GLOBAL_CONFIG = new PublicKey(
  "6s1xP3hpbAfFoNtUNF8mfHsjr2Bd97JxFJRWLbL6aHuX"
);

/** USD1 Global Config */
export const USD1_GLOBAL_CONFIG = new PublicKey(
  "EPiZbnrThjyLnoQ6QQzkxeFqyL5uyg9RzNHHAudUPxBz"
);

/** Event Authority */
export const EVENT_AUTHORITY = new PublicKey(
  "2DPAtwB8L12vrMRExbLuyGnC7n2J5LNoZQSejeQGpwkr"
);

/** WSOL Token Account (mint) */
export const WSOL_TOKEN_ACCOUNT = new PublicKey(
  "So11111111111111111111111111111111111111112"
);

/** USD1 Token Account (mint) */
export const USD1_TOKEN_ACCOUNT = new PublicKey(
  "USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB"
);

/** Fee rates */
export const PLATFORM_FEE_RATE = 100n; // 1%
export const PROTOCOL_FEE_RATE = 25n; // 0.25%
export const SHARE_FEE_RATE = 0n; // 0%

// ============================================
// Discriminators
// ============================================

/** Buy exact in instruction discriminator */
export const BUY_EXACT_IN_DISCRIMINATOR: Buffer = Buffer.from([
  250, 234, 13, 123, 213, 156, 19, 236,
]);

/** Sell exact in instruction discriminator */
export const SELL_EXACT_IN_DISCRIMINATOR: Buffer = Buffer.from([
  149, 39, 222, 155, 211, 124, 152, 26,
]);

// ============================================
// Seeds
// ============================================

export const POOL_SEED = Buffer.from("pool");
export const POOL_VAULT_SEED = Buffer.from("pool_vault");

// ============================================
// PDA Derivation Functions
// ============================================

/**
 * Derive the pool PDA for given base and quote mints
 */
export function getPoolPda(baseMint: PublicKey, quoteMint: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [POOL_SEED, baseMint.toBuffer(), quoteMint.toBuffer()],
    BONK_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive the vault PDA for a pool and mint
 */
export function getVaultPda(poolState: PublicKey, mint: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [POOL_VAULT_SEED, poolState.toBuffer(), mint.toBuffer()],
    BONK_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive platform associated account
 */
export function getPlatformAssociatedAccount(platformConfig: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [platformConfig.toBuffer(), WSOL_TOKEN_ACCOUNT.toBuffer()],
    BONK_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive creator associated account
 */
export function getCreatorAssociatedAccount(creator: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [creator.toBuffer(), WSOL_TOKEN_ACCOUNT.toBuffer()],
    BONK_PROGRAM_ID
  );
  return pda;
}

// ============================================
// Helper Functions
// ============================================

/**
 * Calculate amount in net after fees
 */
export function getAmountInNet(
  amountIn: bigint,
  protocolFeeRate: bigint,
  platformFeeRate: bigint,
  shareFeeRate: bigint
): bigint {
  const protocolFee = (amountIn * protocolFeeRate) / 10000n;
  const platformFee = (amountIn * platformFeeRate) / 10000n;
  const shareFee = (amountIn * shareFeeRate) / 10000n;
  return amountIn - protocolFee - platformFee - shareFee;
}

/**
 * Calculate amount out for a swap
 */
export function getAmountOut(
  amountIn: bigint,
  protocolFeeRate: bigint,
  platformFeeRate: bigint,
  shareFeeRate: bigint,
  virtualBase: bigint,
  virtualQuote: bigint,
  realBase: bigint,
  realQuote: bigint,
  slippageBasisPoints: bigint
): bigint {
  const amountInNet = getAmountInNet(
    amountIn,
    protocolFeeRate,
    platformFeeRate,
    shareFeeRate
  );

  const inputReserve = virtualQuote + realQuote;
  const outputReserve = virtualBase - realBase;

  const numerator = amountInNet * outputReserve;
  const denominator = inputReserve + amountInNet;
  let amountOut = numerator / denominator;

  // Apply slippage
  amountOut = amountOut - (amountOut * slippageBasisPoints) / 10000n;

  return amountOut;
}

/**
 * Calculate amount in required for a desired output
 */
export function getAmountIn(
  amountOut: bigint,
  protocolFeeRate: bigint,
  platformFeeRate: bigint,
  shareFeeRate: bigint,
  virtualBase: bigint,
  virtualQuote: bigint,
  realBase: bigint,
  realQuote: bigint,
  slippageBasisPoints: bigint
): bigint {
  // Consider slippage, actual required output amount is higher
  const amountOutWithSlippage = (amountOut * 10000n) / (10000n - slippageBasisPoints);

  const inputReserve = virtualQuote + realQuote;
  const outputReserve = virtualBase - realBase;

  // Reverse calculate using AMM formula
  const numerator = amountOutWithSlippage * inputReserve;
  const denominator = outputReserve - amountOutWithSlippage;
  const amountInNet = numerator / denominator;

  // Calculate total fee rate
  const totalFeeRate = protocolFeeRate + platformFeeRate + shareFeeRate;

  const amountIn = (amountInNet * 10000n) / (10000n - totalFeeRate);

  return amountIn;
}

// ============================================
// WSOL Helper Functions
// ============================================

/**
 * Create instructions to wrap SOL into WSOL
 */
export function createWsolInstructions(
  payer: PublicKey,
  amount: bigint
): Instruction[] {
  const instructions: Instruction[] = [];
  const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payer, true);

  // Create WSOL ATA if needed (using create account for simplicity)
  // In production, use createAssociatedTokenAccountInstruction

  // Sync native (wrap SOL)
  instructions.push(
    createSyncNativeInstruction(wsolAta)
  );

  return instructions;
}

/**
 * Create instruction to close WSOL ATA and unwrap to SOL
 */
export function createCloseWsolInstruction(payer: PublicKey): Instruction {
  const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payer, true);
  return createCloseAccountInstruction(wsolAta, payer, payer, [], TOKEN_PROGRAM_ID);
}

/**
 * Create WSOL ATA instruction
 */
export function createWsolAtaInstruction(payer: PublicKey): Instruction {
  const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payer, true);
  return createAssociatedTokenAccountInstruction(
    payer,
    wsolAta,
    payer,
    NATIVE_MINT,
    TOKEN_PROGRAM_ID
  );
}

// ============================================
// Types
// ============================================

export interface BonkParams {
  poolState?: PublicKey;
  globalConfig: PublicKey;
  platformConfig: PublicKey;
  platformAssociatedAccount: PublicKey;
  creatorAssociatedAccount: PublicKey;
  baseVault?: PublicKey;
  quoteVault?: PublicKey;
  mintTokenProgram: PublicKey;
  virtualBase: bigint;
  virtualQuote: bigint;
  realBase: bigint;
  realQuote: bigint;
}

export interface BuildBuyInstructionsParams {
  payer: Keypair | PublicKey;
  outputMint: PublicKey;
  inputAmount: bigint;
  slippageBasisPoints?: bigint;
  fixedOutputAmount?: bigint;
  createInputMintAta?: boolean;
  createOutputMintAta?: boolean;
  closeInputMintAta?: boolean;
  protocolParams: BonkParams;
}

export interface BuildSellInstructionsParams {
  payer: Keypair | PublicKey;
  inputMint: PublicKey;
  inputAmount: bigint;
  slippageBasisPoints?: bigint;
  fixedOutputAmount?: bigint;
  createOutputMintAta?: boolean;
  closeOutputMintAta?: boolean;
  closeInputMintAta?: boolean;
  protocolParams: BonkParams;
}

// ============================================
// Instruction Builders
// ============================================

/**
 * Build buy instructions for Bonk protocol
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
    createInputMintAta = true,
    createOutputMintAta = true,
    closeInputMintAta = false,
    protocolParams,
  } = params;

  if (inputAmount === 0n) {
    throw new Error("Amount cannot be zero");
  }

  const payerPubkey = payer instanceof Keypair ? payer.publicKey : payer;
  const instructions: Instruction[] = [];

  const {
    globalConfig,
    platformConfig,
    platformAssociatedAccount,
    creatorAssociatedAccount,
    baseVault,
    quoteVault,
    mintTokenProgram,
    virtualBase,
    virtualQuote,
    realBase,
    realQuote,
  } = protocolParams;

  // Check if USD1 pool
  const isUsd1Pool = globalConfig.equals(USD1_GLOBAL_CONFIG);

  // Determine quote token mint
  const quoteTokenMint = isUsd1Pool ? USD1_TOKEN_ACCOUNT : WSOL_TOKEN_ACCOUNT;

  // Derive pool state
  const poolState = protocolParams.poolState && !protocolParams.poolState.equals(PublicKey.default)
    ? protocolParams.poolState
    : getPoolPda(outputMint, quoteTokenMint);

  // Calculate minimum amount out
  const minimumAmountOut = fixedOutputAmount
    ? fixedOutputAmount
    : getAmountOut(
        inputAmount,
        PROTOCOL_FEE_RATE,
        PLATFORM_FEE_RATE,
        SHARE_FEE_RATE,
        virtualBase,
        virtualQuote,
        realBase,
        realQuote,
        slippageBasisPoints
      );

  // Derive user token accounts
  const userBaseTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    mintTokenProgram
  );
  const userQuoteTokenAccount = getAssociatedTokenAddressSync(
    quoteTokenMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );

  // Derive vault accounts
  const baseVaultAccount = baseVault && !baseVault.equals(PublicKey.default)
    ? baseVault
    : getVaultPda(poolState, outputMint);
  const quoteVaultAccount = quoteVault && !quoteVault.equals(PublicKey.default)
    ? quoteVault
    : getVaultPda(poolState, quoteTokenMint);

  // Handle WSOL wrapping for non-USD1 pools
  if (createInputMintAta && !isUsd1Pool) {
    const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payerPubkey, true);
    instructions.push(
      createAssociatedTokenAccountInstruction(
        payerPubkey,
        wsolAta,
        payerPubkey,
        NATIVE_MINT,
        TOKEN_PROGRAM_ID
      )
    );
    instructions.push(createSyncNativeInstruction(wsolAta));
  }

  // Create output mint ATA if needed
  if (createOutputMintAta) {
    instructions.push(
      createAssociatedTokenAccountInstruction(
        payerPubkey,
        userBaseTokenAccount,
        payerPubkey,
        outputMint,
        mintTokenProgram
      )
    );
  }

  // Build instruction data
  const shareFeeRate = 0n;
  const data = Buffer.alloc(32);
  BUY_EXACT_IN_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(minimumAmountOut, 16);
  data.writeBigUInt64LE(shareFeeRate, 24);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: globalConfig, isSigner: false, isWritable: false },
    { pubkey: platformConfig, isSigner: false, isWritable: false },
    { pubkey: poolState, isSigner: false, isWritable: true },
    { pubkey: userBaseTokenAccount, isSigner: false, isWritable: true },
    { pubkey: userQuoteTokenAccount, isSigner: false, isWritable: true },
    { pubkey: baseVaultAccount, isSigner: false, isWritable: true },
    { pubkey: quoteVaultAccount, isSigner: false, isWritable: true },
    { pubkey: outputMint, isSigner: false, isWritable: false },
    { pubkey: quoteTokenMint, isSigner: false, isWritable: false },
    { pubkey: mintTokenProgram, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: EVENT_AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: BONK_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: platformAssociatedAccount, isSigner: false, isWritable: true },
    { pubkey: creatorAssociatedAccount, isSigner: false, isWritable: true },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: BONK_PROGRAM_ID,
      data,
    })
  );

  // Close WSOL ATA if requested
  if (closeInputMintAta && !isUsd1Pool) {
    const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payerPubkey, true);
    instructions.push(
      createCloseAccountInstruction(wsolAta, payerPubkey, payerPubkey, [], TOKEN_PROGRAM_ID)
    );
  }

  return instructions;
}

/**
 * Build sell instructions for Bonk protocol
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
    createOutputMintAta = true,
    closeOutputMintAta = false,
    closeInputMintAta = false,
    protocolParams,
  } = params;

  if (inputAmount === 0n) {
    throw new Error("Amount cannot be zero");
  }

  const payerPubkey = payer instanceof Keypair ? payer.publicKey : payer;
  const instructions: Instruction[] = [];

  const {
    globalConfig,
    platformConfig,
    platformAssociatedAccount,
    creatorAssociatedAccount,
    baseVault,
    quoteVault,
    mintTokenProgram,
    virtualBase,
    virtualQuote,
    realBase,
    realQuote,
  } = protocolParams;

  // Check if USD1 pool
  const isUsd1Pool = globalConfig.equals(USD1_GLOBAL_CONFIG);

  // Determine quote token mint
  const quoteTokenMint = isUsd1Pool ? USD1_TOKEN_ACCOUNT : WSOL_TOKEN_ACCOUNT;

  // Derive pool state
  const poolState = protocolParams.poolState && !protocolParams.poolState.equals(PublicKey.default)
    ? protocolParams.poolState
    : getPoolPda(inputMint, quoteTokenMint);

  // Calculate minimum amount out
  const minimumAmountOut = fixedOutputAmount
    ? fixedOutputAmount
    : getAmountOut(
        inputAmount,
        PROTOCOL_FEE_RATE,
        PLATFORM_FEE_RATE,
        SHARE_FEE_RATE,
        virtualBase,
        virtualQuote,
        realBase,
        realQuote,
        slippageBasisPoints
      );

  // Derive user token accounts
  const userBaseTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    mintTokenProgram
  );
  const userQuoteTokenAccount = getAssociatedTokenAddressSync(
    quoteTokenMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );

  // Derive vault accounts
  const baseVaultAccount = baseVault && !baseVault.equals(PublicKey.default)
    ? baseVault
    : getVaultPda(poolState, inputMint);
  const quoteVaultAccount = quoteVault && !quoteVault.equals(PublicKey.default)
    ? quoteVault
    : getVaultPda(poolState, quoteTokenMint);

  // Create WSOL ATA for receiving SOL if needed
  if (createOutputMintAta && !isUsd1Pool) {
    const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payerPubkey, true);
    instructions.push(
      createAssociatedTokenAccountInstruction(
        payerPubkey,
        wsolAta,
        payerPubkey,
        NATIVE_MINT,
        TOKEN_PROGRAM_ID
      )
    );
  }

  // Build instruction data
  const shareFeeRate = 0n;
  const data = Buffer.alloc(32);
  SELL_EXACT_IN_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(minimumAmountOut, 16);
  data.writeBigUInt64LE(shareFeeRate, 24);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: globalConfig, isSigner: false, isWritable: false },
    { pubkey: platformConfig, isSigner: false, isWritable: false },
    { pubkey: poolState, isSigner: false, isWritable: true },
    { pubkey: userBaseTokenAccount, isSigner: false, isWritable: true },
    { pubkey: userQuoteTokenAccount, isSigner: false, isWritable: true },
    { pubkey: baseVaultAccount, isSigner: false, isWritable: true },
    { pubkey: quoteVaultAccount, isSigner: false, isWritable: true },
    { pubkey: inputMint, isSigner: false, isWritable: false },
    { pubkey: quoteTokenMint, isSigner: false, isWritable: false },
    { pubkey: mintTokenProgram, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: EVENT_AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: BONK_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: platformAssociatedAccount, isSigner: false, isWritable: true },
    { pubkey: creatorAssociatedAccount, isSigner: false, isWritable: true },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: BONK_PROGRAM_ID,
      data,
    })
  );

  // Close WSOL ATA if requested
  if (closeOutputMintAta && !isUsd1Pool) {
    const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payerPubkey, true);
    instructions.push(
      createCloseAccountInstruction(wsolAta, payerPubkey, payerPubkey, [], TOKEN_PROGRAM_ID)
    );
  }

  // Close input token ATA if requested
  if (closeInputMintAta) {
    instructions.push(
      createCloseAccountInstruction(
        userBaseTokenAccount,
        payerPubkey,
        payerPubkey,
        [],
        mintTokenProgram
      )
    );
  }

  return instructions;
}
