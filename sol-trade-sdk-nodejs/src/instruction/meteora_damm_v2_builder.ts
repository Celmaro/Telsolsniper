/**
 * Meteora DAMM V2 Protocol Instruction Builder
 *
 * Production-grade instruction builder for Meteora DAMM V2 protocol.
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

/** Meteora DAMM V2 program ID */
export const METEORA_DAMM_V2_PROGRAM_ID = new PublicKey(
  "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG"
);

/** Authority */
export const AUTHORITY = new PublicKey(
  "HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC"
);

/** WSOL Token Account (mint) */
export const WSOL_TOKEN_ACCOUNT = new PublicKey(
  "So11111111111111111111111111111111111111112"
);

/** USDC Token Account (mint) */
export const USDC_TOKEN_ACCOUNT = new PublicKey(
  "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
);

// ============================================
// Discriminators
// ============================================

/** Swap instruction discriminator */
export const SWAP_DISCRIMINATOR: Buffer = Buffer.from([
  248, 198, 158, 145, 225, 117, 135, 200,
]);

// ============================================
// Seeds
// ============================================

export const EVENT_AUTHORITY_SEED = Buffer.from("__event_authority");

// ============================================
// PDA Derivation Functions
// ============================================

/**
 * Derive the event authority PDA
 */
export function getEventAuthorityPda(): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [EVENT_AUTHORITY_SEED],
    METEORA_DAMM_V2_PROGRAM_ID
  );
  return pda;
}

// ============================================
// Helper Functions
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

export interface MeteoraDammV2Params {
  pool: PublicKey;
  tokenAMint: PublicKey;
  tokenBMint: PublicKey;
  tokenAVault: PublicKey;
  tokenBVault: PublicKey;
  tokenAProgram: PublicKey;
  tokenBProgram: PublicKey;
}

export interface BuildBuyInstructionsParams {
  payer: Keypair | PublicKey;
  inputMint: PublicKey;
  outputMint: PublicKey;
  inputAmount: bigint;
  slippageBasisPoints?: bigint;
  fixedOutputAmount?: bigint;
  createInputMintAta?: boolean;
  createOutputMintAta?: boolean;
  closeInputMintAta?: boolean;
  protocolParams: MeteoraDammV2Params;
}

export interface BuildSellInstructionsParams {
  payer: Keypair | PublicKey;
  inputMint: PublicKey;
  outputMint: PublicKey;
  inputAmount: bigint;
  slippageBasisPoints?: bigint;
  fixedOutputAmount?: bigint;
  createOutputMintAta?: boolean;
  closeOutputMintAta?: boolean;
  closeInputMintAta?: boolean;
  protocolParams: MeteoraDammV2Params;
}

// ============================================
// Instruction Builders
// ============================================

/**
 * Build buy instructions for Meteora DAMM V2 protocol
 */
export function buildBuyInstructions(
  params: BuildBuyInstructionsParams
): Instruction[] {
  const {
    payer,
    inputMint,
    outputMint,
    inputAmount,
    fixedOutputAmount,
    createInputMintAta = true,
    createOutputMintAta = true,
    closeInputMintAta = false,
    protocolParams,
  } = params;

  if (inputAmount === 0n) {
    throw new Error("Amount cannot be zero");
  }

  if (!fixedOutputAmount) {
    throw new Error("fixedOutputAmount must be set for Meteora DAMM V2 swap");
  }

  const payerPubkey = payer instanceof Keypair ? payer.publicKey : payer;
  const instructions: Instruction[] = [];

  const {
    pool,
    tokenAMint,
    tokenBMint,
    tokenAVault,
    tokenBVault,
    tokenAProgram,
    tokenBProgram,
  } = protocolParams;

  // Check pool type
  const isWsol = tokenAMint.equals(WSOL_TOKEN_ACCOUNT) || tokenBMint.equals(WSOL_TOKEN_ACCOUNT);
  const isUsdc = tokenAMint.equals(USDC_TOKEN_ACCOUNT) || tokenBMint.equals(USDC_TOKEN_ACCOUNT);

  if (!isWsol && !isUsdc) {
    throw new Error("Pool must contain WSOL or USDC");
  }

  // Determine swap direction
  const isAIn = tokenAMint.equals(WSOL_TOKEN_ACCOUNT) || tokenAMint.equals(USDC_TOKEN_ACCOUNT);

  // Derive user token accounts
  const inputTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    isAIn ? tokenAProgram : tokenBProgram
  );
  const outputTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    isAIn ? tokenBProgram : tokenAProgram
  );

  // Derive event authority
  const eventAuthority = getEventAuthorityPda();

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
        TOKEN_PROGRAM_ID
      )
    );
  }

  // Build instruction data
  const data = Buffer.alloc(24);
  SWAP_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(fixedOutputAmount, 16);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: pool, isSigner: false, isWritable: true },
    { pubkey: inputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: outputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: tokenAVault, isSigner: false, isWritable: true },
    { pubkey: tokenBVault, isSigner: false, isWritable: true },
    { pubkey: tokenAMint, isSigner: false, isWritable: false },
    { pubkey: tokenBMint, isSigner: false, isWritable: false },
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: tokenAProgram, isSigner: false, isWritable: false },
    { pubkey: tokenBProgram, isSigner: false, isWritable: false },
    { pubkey: METEORA_DAMM_V2_PROGRAM_ID, isSigner: false, isWritable: false }, // Referral Token Account (placeholder)
    { pubkey: eventAuthority, isSigner: false, isWritable: false },
    { pubkey: METEORA_DAMM_V2_PROGRAM_ID, isSigner: false, isWritable: false },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: METEORA_DAMM_V2_PROGRAM_ID,
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
 * Build sell instructions for Meteora DAMM V2 protocol
 */
export function buildSellInstructions(
  params: BuildSellInstructionsParams
): Instruction[] {
  const {
    payer,
    inputMint,
    outputMint,
    inputAmount,
    fixedOutputAmount,
    createOutputMintAta = true,
    closeOutputMintAta = false,
    closeInputMintAta = false,
    protocolParams,
  } = params;

  if (inputAmount === 0n) {
    throw new Error("Amount cannot be zero");
  }

  if (!fixedOutputAmount) {
    throw new Error("fixedOutputAmount must be set for Meteora DAMM V2 swap");
  }

  const payerPubkey = payer instanceof Keypair ? payer.publicKey : payer;
  const instructions: Instruction[] = [];

  const {
    pool,
    tokenAMint,
    tokenBMint,
    tokenAVault,
    tokenBVault,
    tokenAProgram,
    tokenBProgram,
  } = protocolParams;

  // Check pool type
  const isWsol = tokenBMint.equals(WSOL_TOKEN_ACCOUNT) || tokenAMint.equals(WSOL_TOKEN_ACCOUNT);
  const isUsdc = tokenBMint.equals(USDC_TOKEN_ACCOUNT) || tokenAMint.equals(USDC_TOKEN_ACCOUNT);

  if (!isWsol && !isUsdc) {
    throw new Error("Pool must contain WSOL or USDC");
  }

  // Determine swap direction (selling token for WSOL/USDC)
  const isAIn = tokenBMint.equals(WSOL_TOKEN_ACCOUNT) || tokenBMint.equals(USDC_TOKEN_ACCOUNT);

  // Derive user token accounts
  const inputTokenAccount = getAssociatedTokenAddressSync(
    inputMint,
    payerPubkey,
    true,
    isAIn ? tokenAProgram : tokenBProgram
  );
  const outputTokenAccount = getAssociatedTokenAddressSync(
    outputMint,
    payerPubkey,
    true,
    isAIn ? tokenBProgram : tokenAProgram
  );

  // Derive event authority
  const eventAuthority = getEventAuthorityPda();

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
  SWAP_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(inputAmount, 8);
  data.writeBigUInt64LE(fixedOutputAmount, 16);

  // Build accounts
  const accounts: AccountMeta[] = [
    { pubkey: AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: pool, isSigner: false, isWritable: true },
    { pubkey: inputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: outputTokenAccount, isSigner: false, isWritable: true },
    { pubkey: tokenAVault, isSigner: false, isWritable: true },
    { pubkey: tokenBVault, isSigner: false, isWritable: true },
    { pubkey: tokenAMint, isSigner: false, isWritable: false },
    { pubkey: tokenBMint, isSigner: false, isWritable: false },
    { pubkey: payerPubkey, isSigner: true, isWritable: true },
    { pubkey: tokenAProgram, isSigner: false, isWritable: false },
    { pubkey: tokenBProgram, isSigner: false, isWritable: false },
    { pubkey: METEORA_DAMM_V2_PROGRAM_ID, isSigner: false, isWritable: false }, // Referral Token Account
    { pubkey: eventAuthority, isSigner: false, isWritable: false },
    { pubkey: METEORA_DAMM_V2_PROGRAM_ID, isSigner: false, isWritable: false },
  ];

  instructions.push(
    new Instruction({
      keys: accounts,
      programId: METEORA_DAMM_V2_PROGRAM_ID,
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
        isAIn ? tokenAProgram : tokenBProgram
      )
    );
  }

  return instructions;
}
