import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  TOKEN_2022_PROGRAM_ID,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  createTransferCheckedWithTransferHookInstruction,
} from "@solana/spl-token";
import { Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { RwaCompliance } from "../target/types/rwa_compliance";
import { RwaTransferHook } from "../target/types/rwa_transfer_hook";
import { createComplianceMint, investorPda, configPda, DECIMALS, ONE_DAY, now } from "./utils";

describe("02: happy path — KYC'd investors can transfer under compliance rules", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const compliance = anchor.workspace.RwaCompliance as Program<RwaCompliance>;
  const hook = anchor.workspace.RwaTransferHook as Program<RwaTransferHook>;
  const payer = (provider.wallet as anchor.Wallet).payer;

  it("allows a transfer between two allowlisted, KYC'd, thawed investors", async () => {
    const agent = anchor.web3.Keypair.generate();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const { mint } = await createComplianceMint(
      provider.connection,
      payer,
      compliance.programId,
      hook.programId
    );
    const [config] = configPda(compliance.programId, mint.publicKey);

    await compliance.methods
      .initializeConfig(agent.publicKey, new anchor.BN(0), 0)
      .accounts({ authority: payer.publicKey, mint: mint.publicKey })
      .rpc();

    // Allow jurisdiction 1 (arbitrary code standing in for e.g. "US")
    await compliance.methods
      .manageJurisdictions(1, true)
      .accounts({ authority: payer.publicKey, config })
      .rpc();

    await hook.methods
      .initializeExtraAccountMetaList()
      .accounts({ payer: payer.publicKey, mint: mint.publicKey, complianceProgram: compliance.programId })
      .rpc();

    const kycExpiry = now() + 365 * ONE_DAY;

    for (const investor of [alice, bob]) {
      await compliance.methods
        .upsertInvestor(investor.publicKey, 1, new anchor.BN(kycExpiry), new anchor.BN(0), new anchor.BN(0), false)
        .accounts({ agent: agent.publicKey, config })
        .signers([agent])
        .rpc();
    }

    const aliceAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint.publicKey,
      alice.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    const bobAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint.publicKey,
      bob.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    for (const [investor, ata] of [
      [alice, aliceAta],
      [bob, bobAta],
    ] as const) {
      await compliance.methods
        .thawInvestorAccount()
        .accounts({
          agent: agent.publicKey,
          config,
          investorRecord: investorPda(compliance.programId, mint.publicKey, investor.publicKey)[0],
          mint: mint.publicKey,
          tokenAccount: ata.address,
        })
        .signers([agent])
        .rpc();
    }

    const mintAmount = 1_000 * 10 ** DECIMALS;
    await mintTo(
      provider.connection,
      payer,
      mint.publicKey,
      aliceAta.address,
      payer,
      mintAmount,
      [],
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    const transferAmount = 100 * 10 ** DECIMALS;
    const ix = await createTransferCheckedWithTransferHookInstruction(
      provider.connection,
      aliceAta.address,
      mint.publicKey,
      bobAta.address,
      alice.publicKey,
      BigInt(transferAmount),
      DECIMALS,
      [],
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    const tx = new Transaction().add(ix);
    await sendAndConfirmTransaction(provider.connection, tx, [payer, alice]);

    const bobAccount = await getAccount(provider.connection, bobAta.address, "confirmed", TOKEN_2022_PROGRAM_ID);
    expect(bobAccount.amount.toString()).to.equal(BigInt(transferAmount).toString());
  });
});
