/**
 * Raydium CPMM (Concentrated Pool Market Maker) Protocol Instruction Builder
 *
 * Production-grade instruction builder for Raydium CPMM protocol.
 * Supports swap operations with WSOL and USDC pools.
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
} from "@solana/spl-token";

// ============================================
// Program IDs and Constants
// ============================================

/** Raydium CPMM program ID */
export const RAYDIUM_CPMM_PROGRAM_ID = new PublicKey(
  "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C"
);

/** Authority */
export const AUTHORITY = new PublicKey(
  "GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL"
);

/** WSOL Token Account (mint) */
export const WSOL_TOKEN_ACCOUNT = new PublicKey(
  "So11111111111111111111111111111111111111112"
);

/** USDC Token Account (mint) */
export const USDC_TOKEN_ACCOUNT = new PublicKey(
  "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
);

/** Fee rates */
export const FEE_RATE_DENOMINATOR_VALUE = 1_000_000n;
export const TRADE_FEE_RATE = 2500n;
export const CREATOR_FEE_RATE = 0n;
export const PROTOCOL_FEE_RATE = 120000n;
export const FUND_FEE_RATE = 40000n;

// ============================================
// Discriminators
// ============================================

/** Swap base in instruction discriminator */
export const SWAP_BASE_IN_DISCRIMINATOR: Buffer = Buffer.from([
  143, 190, 90, 218, 196, 30, 51, 222,
]);

/** Swap base out instruction discriminator */
export const SWAP_BASE_OUT_DISCRIMINATOR: Buffer = Buffer.from([
  55, 217, 98, 86, 163, 74, 180, 173,
]);

// ============================================
// Seeds
// ============================================

export const POOL_SEED = Buffer.from("pool");
export const POOL_VAULT_SEED = Buffer.from("pool_vault");
export const OBSERVATION_STATE_SEED = Buffer.from("observation");

// ============================================
// PDA Derivation Functions
// ============================================

/**
 * Derive the pool PDA for given config and mints
 */
export function getPoolPda(
  ammConfig: PublicKey,
  mint1: PublicKey,
  mint2: PublicKey
): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [POOL_SEED, ammConfig.toBuffer(), mint1.toBuffer(), mint2.toBuffer()],
    RAYDIUM_CPMM_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive the vault PDA for a pool and mint
 */
export function getVaultPda(poolState: PublicKey, mint: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [POOL_VAULT_SEED, poolState.toBuffer(), mint.toBuffer()],
    RAYDIUM_CPMM_PROGRAM_ID
  );
  return pda;
}

/**
 * Derive the observation state PDA for a pool
 */
export function getObservationStatePda(poolState: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [OBSERVATION_STATE_SEED, poolState.toBuffer()],
    RAYDIUM_CPMM_PROGRAM_ID
  );
  return pda;
}

// ============================================
// Helper Functions
// ============================================

/**
 * Compute swap amount for CPMM
 */
export function computeSwapAmount(
  baseReserve: bigint,
  quoteReserve: bigint,
  isBaseIn: boolean,
  amountIn: bigint,
  slippageBasisPoints: bigint
): { amountOut: bigint; minAmountOut: bigint } {
  // Apply trade fee (0.25%)
  const feeRate = TRADE_FEE_RATE;
  const feeDenominator = FEE_RATE_DENOMINATOR_VALUE;
  const amountInAfterFee = amountIn - (amountIn * feeRate) / feeDenominator;

  // Calculate output using constant product formula
  let amountOut: bigint;
  if (isBaseIn) {
    // Selling base for quote: output = (quoteReserve * amountIn) / (baseReserve + amountIn)
    const denominator = baseReserve + amountInAfterFee;
    amountOut = (quoteReserve * amountInAfterFee) / denominator;
  } else {
    // Selling quote for base: output = (baseReserve * amountIn) / (quoteReserve + amountIn)
    const denominator = quoteReserve + amountInAfterFee;
    amountOut = (baseReserve * amountInAfterFee) / denominator;
  }

  // Apply slippage
  const minAmountOut = amountOut - (amountOut * slippageBasisPoints) / 10000n;

  return { amountOut, minAmountOut };
}

/**
 * Create instructions to wrap SOL into WSOL
 */
export function createWsolInstructions(
  payer: PublicKey,
  amount: bigint
): Instruction[] {
  const instructions: Instruction[] = [];
  const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payer, true);

  // Sync native (wrap SOL)
  instructions.push(createSyncNativeInstruction(wsolAta));

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

export interface RaydiumCpmmParams {
  poolState?: PublicKey;
  ammConfig: PublicKey;
  baseMint: PublicKey;
  quoteMint: PublicKey;
  baseTokenProgram: PublicKey;
  quoteTokenProgram: PublicKey;
  baseVault?: PublicKey;
  quoteVault?: PublicKey;
  baseReserve: bigint;
  quoteReserve: bigint;
  observationState?: PublicKey;
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
  protocolParams: RaydiumCpmmParams;
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
  protocolParams: RaydiumCpmmParams;
}

// ============================================
// Instruction Builders
// ============================================

/**
 * Build buy instructions for Raydium CPMM protocol
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
    ammConfig,
    baseMint,
    quoteMint,
    baseTokenProgram,
    quoteTokenProgram,
    baseVault,
    quoteVault,
    baseReserve,
    quoteReserve,
    observationState,
  } = protocolParams;

  // Check pool type
  const isWsol = baseMint.equals(WSOL_TOKEN_ACCOUNT) || quoteMint.equals(WSOL_TOKEN_ACCOUNT);
  const isUsdc = baseMint.equals(USDC_TOKEN_ACCOUNT) || quoteMint.equals(USDC_TOKEN_ACCOUNT);

  if (!isWsol && !isUsdc) {
    throw new Error("Pool must contain WSOL or USDC");
  }

  // Determine swap direction
  const isBaseIn = baseMint.equals(WSOL_TOKEN_ACCOUNT) || baseMint.equals(USDC_TOKEN_ACCOUNT);
  const mintTokenProgram = isBaseIn ? quoteTokenProgram : baseTokenProgram;

  // Derive pool state
  const poolState = protocolParams.poolState && !protocolParams.poolState.equals(PublicKey.default)
    ? protocolParams.poolState
    : getPoolPda(ammConfig, baseMint, quoteMint);

  // Calculate output
  const swapResult = computeSwapAmount(
    baseReserve,
    quoteReserve,
    isBaseIn,
    inputAmount,
    slippageBasisPoints
  );
  const minimumAmountOut = fixedOutputAmount || swapResult.minAmountOut;

  // Determine input/output mints
  const inputMint = isWsol ? WSOL_TOKEN_ACCOUNT : USDC_TOKEN_ACCOUNT;

  // Derive user token accounts
  const inputTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );
  const outputTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    mintTokenProgram
  );

  // Derive vault accounts
  const inputVaultAccount = getVaultPda(poolState, inputMint);
  const outputVaultAccount = getVaultPda(poolState, outputMint);

  // Derive observation state
  const observationStateAccount = observationState && !observationState.equals(PublicKey.default)
    ? observationState
    : getObservationStatePda(poolState);

  // Handle WSOL wrapping
  if (createInputMintAta && isWsol) {
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
        outputTokenAccount,
        payerPubkey,
        outputMint,
        mintTokenProgram
      )
    );
  }

  // Build instruction data
  const data = Buffer.alloc(24);
  SWAP_BASE_IN_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(minimumAmountOut, 16);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: ammConfig, isSigner: false, isWritable: false },
    { pubkey: poolState, isSigner: false, isWritable: true },
    { pubkey: inputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: outputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: inputVaultAccount, isSigner: false, isWritable: true },
    { pubkey: outputVaultAccount, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: mintTokenProgram, isSigner: false, isWritable: false },
    { pubkey: inputMint, isSigner: false, isWritable: false },
    { pubkey: outputMint, isSigner: false, isWritable: false },
    { pubkey: observationStateAccount, isSigner: false, isWritable: true },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: RAYDIUM_CPMM_PROGRAM_ID,
      data,
    })
  );

  // Close WSOL ATA if requested
  if (closeInputMintAta && isWsol) {
    const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payerPubkey, true);
    instructions.push(
      createCloseAccountInstruction(wsolAta, payerPubkey, payerPubkey, [], TOKEN_PROGRAM_ID)
    );
  }

  return instructions;
}

/**
 * Build sell instructions for Raydium CPMM protocol
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
    ammConfig,
    baseMint,
    quoteMint,
    baseTokenProgram,
    quoteTokenProgram,
    baseReserve,
    quoteReserve,
    observationState,
  } = protocolParams;

  // Check pool type
  const isWsol = baseMint.equals(WSOL_TOKEN_ACCOUNT) || quoteMint.equals(WSOL_TOKEN_ACCOUNT);
  const isUsdc = baseMint.equals(USDC_TOKEN_ACCOUNT) || quoteMint.equals(USDC_TOKEN_ACCOUNT);

  if (!isWsol && !isUsdc) {
    throw new Error("Pool must contain WSOL or USDC");
  }

  // Determine swap direction
  const isQuoteOut = quoteMint.equals(WSOL_TOKEN_ACCOUNT) || quoteMint.equals(USDC_TOKEN_ACCOUNT);
  const mintTokenProgram = isQuoteOut ? baseTokenProgram : quoteTokenProgram;

  // Derive pool state
  const poolState = protocolParams.poolState && !protocolParams.poolState.equals(PublicKey.default)
    ? protocolParams.poolState
    : getPoolPda(ammConfig, baseMint, quoteMint);

  // Calculate output
  const swapResult = computeSwapAmount(
    baseReserve,
    quoteReserve,
    isQuoteOut,
    inputAmount,
    slippageBasisPoints
  );
  const minimumAmountOut = fixedOutputAmount || swapResult.minAmountOut;

  // Determine output mint
  const outputMint = isWsol ? WSOL_TOKEN_ACCOUNT : USDC_TOKEN_ACCOUNT;

  // Derive user token accounts
  const inputTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    mintTokenProgram
  );
  const outputTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );

  // Derive vault accounts
  const inputVaultAccount = getVaultPda(poolState, inputMint);
  const outputVaultAccount = getVaultPda(poolState, outputMint);

  // Derive observation state
  const observationStateAccount = observationState && !observationState.equals(PublicKey.default)
    ? observationState
    : getObservationStatePda(poolState);

  // Create WSOL ATA for receiving if needed
  if (createOutputMintAta && isWsol) {
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
  const data = Buffer.alloc(24);
  SWAP_BASE_IN_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(minimumAmountOut, 16);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: ammConfig, isSigner: false, isWritable: false },
    { pubkey: poolState, isSigner: false, isWritable: true },
    { pubkey: inputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: outputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: inputVaultAccount, isSigner: false, isWritable: true },
    { pubkey: outputVaultAccount, isSigner: false, isWritable: true },
    { pubkey: mintTokenProgram, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: inputMint, isSigner: false, isWritable: false },
    { pubkey: outputMint, isSigner: false, isWritable: false },
    { pubkey: observationStateAccount, isSigner: false, isWritable: true },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: RAYDIUM_CPMM_PROGRAM_ID,
      data,
    })
  );

  // Close WSOL ATA if requested
  if (closeOutputMintAta && isWsol) {
    const wsolAta = getAssociatedTokenAddressSync(NATIVE_MINT, payerPubkey, true);
    instructions.push(
      createCloseAccountInstruction(wsolAta, payerPubkey, payerPubkey, [], TOKEN_PROGRAM_ID)
    );
  }

  // Close input token ATA if requested
  if (closeInputMintAta) {
    instructions.push(
      createCloseAccountInstruction(
        inputTokenAccount,
        payerPubkey,
        payerPubkey,
        [],
        mintTokenProgram
      )
    );
  }

  return instructions;
}
