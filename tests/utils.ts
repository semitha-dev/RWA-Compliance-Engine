import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  ExtensionType,
  TOKEN_2022_PROGRAM_ID,
  createInitializeMintInstruction,
  createInitializeDefaultAccountStateInstruction,
  createInitializePermanentDelegateInstruction,
  createInitializeTransferHookInstruction,
  getMintLen,
  AccountState,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { RwaCompliance } from "../target/types/rwa_compliance";
import { RwaTransferHook } from "../target/types/rwa_transfer_hook";

export const DECIMALS = 6;

export function configPda(programId: PublicKey, mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([Buffer.from("config"), mint.toBuffer()], programId);
}

export function investorPda(
  programId: PublicKey,
  mint: PublicKey,
  owner: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("investor"), mint.toBuffer(), owner.toBuffer()],
    programId
  );
}

export function extraAccountMetaListPda(hookProgramId: PublicKey, mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("extra-account-metas"), mint.toBuffer()],
    hookProgramId
  );
}

/**
 * Creates a Token-2022 mint wired up for on-chain compliance:
 * - Default Account State = Frozen, so every fresh ATA starts unable to receive tokens
 *   until `thaw_investor_account` is called.
 * - Permanent Delegate = the compliance Config PDA, enabling `force_transfer` clawback.
 * - Freeze Authority = the compliance Config PDA, enabling `thaw_investor_account` /
 *   `freeze_investor_account`.
 * - Transfer Hook = the rwa-transfer-hook program, enforcing compliance on every transfer.
 */
export async function createComplianceMint(
  connection: anchor.web3.Connection,
  payer: Keypair,
  complianceProgramId: PublicKey,
  hookProgramId: PublicKey
): Promise<{ mint: Keypair; configAddress: PublicKey }> {
  const mint = Keypair.generate();
  const [configAddress] = configPda(complianceProgramId, mint.publicKey);

  const extensions = [
    ExtensionType.DefaultAccountState,
    ExtensionType.PermanentDelegate,
    ExtensionType.TransferHook,
  ];
  const mintLen = getMintLen(extensions);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: mint.publicKey,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    }),
    createInitializeDefaultAccountStateInstruction(
      mint.publicKey,
      AccountState.Frozen,
      TOKEN_2022_PROGRAM_ID
    ),
    createInitializePermanentDelegateInstruction(mint.publicKey, configAddress, TOKEN_2022_PROGRAM_ID),
    createInitializeTransferHookInstruction(
      mint.publicKey,
      configAddress,
      hookProgramId,
      TOKEN_2022_PROGRAM_ID
    ),
    createInitializeMintInstruction(mint.publicKey, DECIMALS, configAddress, configAddress, TOKEN_2022_PROGRAM_ID)
  );

  await sendAndConfirmTransaction(connection, tx, [payer, mint]);

  return { mint, configAddress };
}

export const ONE_DAY = 24 * 60 * 60;

export function now(): number {
  return Math.floor(Date.now() / 1000);
}
