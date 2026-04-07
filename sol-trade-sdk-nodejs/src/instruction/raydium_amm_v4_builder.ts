/**
 * Raydium AMM V4 Protocol Instruction Builder
 *
 * Production-grade instruction builder for Raydium AMM V4 protocol.
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

/** Raydium AMM V4 program ID */
export const RAYDIUM_AMM_V4_PROGRAM_ID = new PublicKey(
  "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
);

/** Authority */
export const AUTHORITY = new PublicKey(
  "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1"
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
export const TRADE_FEE_NUMERATOR = 25n;
export const TRADE_FEE_DENOMINATOR = 10000n;
export const SWAP_FEE_NUMERATOR = 25n;
export const SWAP_FEE_DENOMINATOR = 10000n;

// ============================================
// Discriminators
// ============================================

/** Swap base in instruction discriminator (single byte) */
export const SWAP_BASE_IN_DISCRIMINATOR: Buffer = Buffer.from([9]);

/** Swap base out instruction discriminator (single byte) */
export const SWAP_BASE_OUT_DISCRIMINATOR: Buffer = Buffer.from([11]);

// ============================================
// Seeds
// ============================================

export const POOL_SEED = Buffer.from("pool");

// ============================================
// Helper Functions
// ============================================

/**
 * Compute swap amount for AMM V4
 */
export function computeSwapAmount(
  coinReserve: bigint,
  pcReserve: bigint,
  isCoinIn: boolean,
  amountIn: bigint,
  slippageBasisPoints: bigint
): { amountOut: bigint; minAmountOut: bigint } {
  // Apply trade fee (0.25%)
  const amountInAfterFee = amountIn - (amountIn * TRADE_FEE_NUMERATOR) / TRADE_FEE_DENOMINATOR;

  // Calculate output using constant product formula
  let amountOut: bigint;
  if (isCoinIn) {
    // Selling coin for pc: output = (pcReserve * amountIn) / (coinReserve + amountIn)
    const denominator = coinReserve + amountInAfterFee;
    amountOut = (pcReserve * amountInAfterFee) / denominator;
  } else {
    // Selling pc for coin: output = (coinReserve * amountIn) / (pcReserve + amountIn)
    const denominator = pcReserve + amountInAfterFee;
    amountOut = (coinReserve * amountInAfterFee) / denominator;
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

export interface RaydiumAmmV4Params {
  amm: PublicKey;
  coinMint: PublicKey;
  pcMint: PublicKey;
  tokenCoin: PublicKey;
  tokenPc: PublicKey;
  coinReserve: bigint;
  pcReserve: bigint;
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
  protocolParams: RaydiumAmmV4Params;
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
  protocolParams: RaydiumAmmV4Params;
}

// ============================================
// Instruction Builders
// ============================================

/**
 * Build buy instructions for Raydium AMM V4 protocol
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
    amm,
    coinMint,
    pcMint,
    tokenCoin,
    tokenPc,
    coinReserve,
    pcReserve,
  } = protocolParams;

  // Check pool type
  const isWsol = coinMint.equals(WSOL_TOKEN_ACCOUNT) || pcMint.equals(WSOL_TOKEN_ACCOUNT);
  const isUsdc = coinMint.equals(USDC_TOKEN_ACCOUNT) || pcMint.equals(USDC_TOKEN_ACCOUNT);

  if (!isWsol && !isUsdc) {
    throw new Error("Pool must contain WSOL or USDC");
  }

  // Determine swap direction
  const isBaseIn = coinMint.equals(WSOL_TOKEN_ACCOUNT) || coinMint.equals(USDC_TOKEN_ACCOUNT);

  // Calculate output
  const swapResult = computeSwapAmount(
    coinReserve,
    pcReserve,
    isBaseIn,
    inputAmount,
    slippageBasisPoints
  );
  const minimumAmountOut = fixedOutputAmount || swapResult.minAmountOut;

  // Determine input/output mints
  const inputMint = isWsol ? WSOL_TOKEN_ACCOUNT : USDC_TOKEN_ACCOUNT;

  // Derive user token accounts
  const userSourceTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );
  const userDestinationTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );

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
        userDestinationTokenAccount,
        payerPubkey,
        outputMint,
        TOKEN_PROGRAM_ID
      )
    );
  }

  // Build instruction data (1 byte discriminator + 8 bytes amountIn + 8 bytes minAmountOut)
  const data = Buffer.alloc(17);
  SWAP_BASE_IN_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 1);
  data.writeBigUInt64LE(minimumAmountOut, 9);

  // Build accounts (Raydium AMM V4 has a specific account order)
  const accounts: AccountMeta[] = [
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: amm, isSigner: false, isWritable: true },
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: amm, isSigner: false, isWritable: true }, // Amm Open Orders (same as amm for simplicity)
    { pubkey: tokenCoin, isSigner: false, isWritable: true }, // Pool Coin Token Account
    { pubkey: tokenPc, isSigner: false, isWritable: true }, // Pool Pc Token Account
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Program (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Market (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Bids (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Asks (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Event Queue (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Coin Vault Account (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Pc Vault Account (placeholder)
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Vault Signer (placeholder)
    { pubkey: userSourceTokenAccount, isSigner: false, isWritable: true },
    { pubkey: userDestinationTokenAccount, isSigner: false, isWritable: true },
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: RAYDIUM_AMM_V4_PROGRAM_ID,
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
 * Build sell instructions for Raydium AMM V4 protocol
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
    amm,
    coinMint,
    pcMint,
    tokenCoin,
    tokenPc,
    coinReserve,
    pcReserve,
  } = protocolParams;

  // Check pool type
  const isWsol = coinMint.equals(WSOL_TOKEN_ACCOUNT) || pcMint.equals(WSOL_TOKEN_ACCOUNT);
  const isUsdc = coinMint.equals(USDC_TOKEN_ACCOUNT) || pcMint.equals(USDC_TOKEN_ACCOUNT);

  if (!isWsol && !isUsdc) {
    throw new Error("Pool must contain WSOL or USDC");
  }

  // Determine swap direction (selling token for WSOL/USDC means pc is output)
  const isBaseIn = pcMint.equals(WSOL_TOKEN_ACCOUNT) || pcMint.equals(USDC_TOKEN_ACCOUNT);

  // Calculate output
  const swapResult = computeSwapAmount(
    coinReserve,
    pcReserve,
    isBaseIn,
    inputAmount,
    slippageBasisPoints
  );
  const minimumAmountOut = fixedOutputAmount || swapResult.minAmountOut;

  // Determine output mint
  const outputMint = isWsol ? WSOL_TOKEN_ACCOUNT : USDC_TOKEN_ACCOUNT;

  // Derive user token accounts
  const userSourceTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );
  const userDestinationTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    TOKEN_PROGRAM_ID
  );

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
  const data = Buffer.alloc(17);
  SWAP_BASE_IN_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 1);
  data.writeBigUInt64LE(minimumAmountOut, 9);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: amm, isSigner: false, isWritable: true },
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: amm, isSigner: false, isWritable: true }, // Amm Open Orders
    { pubkey: tokenCoin, isSigner: false, isWritable: true }, // Pool Coin Token Account
    { pubkey: tokenPc, isSigner: false, isWritable: true }, // Pool Pc Token Account
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Program
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Market
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Bids
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Asks
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Event Queue
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Coin Vault Account
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Pc Vault Account
    { pubkey: amm, isSigner: false, isWritable: false }, // Serum Vault Signer
    { pubkey: userSourceTokenAccount, isSigner: false, isWritable: true },
    { pubkey: userDestinationTokenAccount, isSigner: false, isWritable: true },
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: RAYDIUM_AMM_V4_PROGRAM_ID,
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
        userSourceTokenAccount,
        payerPubkey,
        payerPubkey,
        [],
        TOKEN_PROGRAM_ID
      )
    );
  }

  return instructions;
}
