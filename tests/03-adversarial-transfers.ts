import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  TOKEN_2022_PROGRAM_ID,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  createTransferCheckedWithTransferHookInstruction,
} from "@solana/spl-token";
import { Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { RwaCompliance } from "../target/types/rwa_compliance";
import { RwaTransferHook } from "../target/types/rwa_transfer_hook";
import { createComplianceMint, investorPda, configPda, extraAccountMetaListPda, DECIMALS, ONE_DAY, now } from "./utils";

/**
 * These tests are the point of the project: they prove the compliance engine actually
 * rejects the transfers it claims to reject, not just that the happy path works. Each
 * one sets up the minimum state needed to isolate a single violated rule.
 */
describe("03: adversarial — every rejection path is actually enforced", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const compliance = anchor.workspace.RwaCompliance as Program<RwaCompliance>;
  const hook = anchor.workspace.RwaTransferHook as Program<RwaTransferHook>;
  const payer = (provider.wallet as anchor.Wallet).payer;

  async function setupMintWithConfig(lockupUntil = 0, maxHolders = 0) {
    const agent = anchor.web3.Keypair.generate();
    const { mint } = await createComplianceMint(provider.connection, payer, compliance.programId, hook.programId);
    const [config] = configPda(compliance.programId, mint.publicKey);

    await compliance.methods
      .initializeConfig(agent.publicKey, new anchor.BN(lockupUntil), maxHolders)
      .accounts({ authority: payer.publicKey, mint: mint.publicKey })
      .rpc();

    await compliance.methods
      .manageJurisdictions(1, true)
      .accounts({ authority: payer.publicKey, config })
      .rpc();

    await hook.methods
      .initializeExtraAccountMetaList()
      .accounts({ payer: payer.publicKey, mint: mint.publicKey, complianceProgram: compliance.programId })
      .rpc();

    return { agent, mint, config };
  }

  async function kycAndThaw(
    agent: anchor.web3.Keypair,
    mint: anchor.web3.PublicKey,
    config: anchor.web3.PublicKey,
    investor: anchor.web3.Keypair,
    opts: { kycExpiry?: number; lockupUntil?: number; maxBalance?: number; countryCode?: number } = {}
  ) {
    await compliance.methods
      .upsertInvestor(
        investor.publicKey,
        opts.countryCode ?? 1,
        new anchor.BN(opts.kycExpiry ?? now() + 365 * ONE_DAY),
        new anchor.BN(opts.lockupUntil ?? 0),
        new anchor.BN(opts.maxBalance ?? 0),
        false
      )
      .accounts({ agent: agent.publicKey, config })
      .signers([agent])
      .rpc();

    const ata = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint,
      investor.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await compliance.methods
      .thawInvestorAccount()
      .accounts({
        agent: agent.publicKey,
        config,
        investorRecord: investorPda(compliance.programId, mint, investor.publicKey)[0],
        mint,
        tokenAccount: ata.address,
      })
      .signers([agent])
      .rpc();

    return ata;
  }

  async function attemptTransfer(
    mint: anchor.web3.PublicKey,
    fromAta: anchor.web3.PublicKey,
    toAta: anchor.web3.PublicKey,
    fromSigner: anchor.web3.Keypair,
    amount: number
  ): Promise<{ ok: boolean; error?: string }> {
    try {
      const ix = await createTransferCheckedWithTransferHookInstruction(
        provider.connection,
        fromAta,
        mint,
        toAta,
        fromSigner.publicKey,
        BigInt(amount),
        DECIMALS,
        [],
        "confirmed",
        TOKEN_2022_PROGRAM_ID
      );
      const tx = new Transaction().add(ix);
      await sendAndConfirmTransaction(provider.connection, tx, [payer, fromSigner]);
      return { ok: true };
    } catch (e: any) {
      return { ok: false, error: e?.toString?.() ?? String(e) };
    }
  }

  it("rejects a transfer to a recipient with no InvestorRecord at all (never allowlisted)", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const strangerNotKyced = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    // Stranger's ATA exists (so the transfer instruction can be built) but they were
    // never KYC'd, so their InvestorRecord PDA doesn't exist -> account resolution
    // for the hook's extra accounts must fail before the hook logic even runs.
    const strangerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint.publicKey,
      strangerNotKyced.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, strangerAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer to a non-KYC'd, non-thawed recipient must fail");
  });

  it("rejects a transfer where the destination InvestorRecord has expired KYC", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    // Bob's KYC "expired" 1 day ago.
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob, { kycExpiry: now() - ONE_DAY });

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, bobAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer to an investor with expired KYC must fail");
  });

  it("rejects direct invocation of `execute` outside of an active Token-2022 transfer", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob);

    const [aliceRecord] = investorPda(compliance.programId, mint.publicKey, alice.publicKey);
    const [bobRecord] = investorPda(compliance.programId, mint.publicKey, bob.publicKey);
    const [extraMetas] = extraAccountMetaListPda(hook.programId, mint.publicKey);

    // Calling `execute` directly (not as a Token-2022 CPI mid-transfer) must be rejected
    // by the `is_transferring` flag check -- this is the exact bypass the check exists
    // to prevent: an attacker skipping Token-2022 entirely and invoking the hook's
    // enforcement instruction as if a transfer were happening, hoping it just logs
    // and returns Ok(()) like a naive implementation would.
    let threw = false;
    try {
      await hook.methods
        .execute(new anchor.BN(1))
        .accounts({
          sourceToken: aliceAta.address,
          mint: mint.publicKey,
          destinationToken: bobAta.address,
          owner: alice.publicKey,
          extraAccountMetaList: extraMetas,
          config,
          sourceInvestor: aliceRecord,
          destinationInvestor: bobRecord,
        })
        .rpc();
    } catch (_e) {
      threw = true;
    }
    expect(threw).to.equal(true, "direct Execute invocation must be rejected");
  });

  it("rejects a transfer while the mint-wide global lockup is still active", async () => {
    const { agent, mint, config } = await setupMintWithConfig(now() + 365 * ONE_DAY);
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, bobAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer during an active global lockup must fail");
  });

  it("rejects a transfer to a jurisdiction not on the mint's allowlist", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    // Bob is KYC'd but tagged with country_code 99, which was never added to the
    // mint's allowed_jurisdictions bitmap (only code 1 was allowed in setupMintWithConfig).
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob, { countryCode: 99 });

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, bobAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer to a disallowed jurisdiction must fail");
  });

  it("rejects a transfer that would exceed the destination's balance cap", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    // Bob is capped at 5 tokens; Alice attempts to send 10.
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob, { maxBalance: 5 * 10 ** DECIMALS });

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, bobAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer exceeding destination's balance cap must fail");
  });

  it("rejects a transfer while the mint is paused", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    await compliance.methods.setPaused(true).accounts({ authority: payer.publicKey, config }).rpc();

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, bobAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer while paused must fail");
  });

  it("rejects a transfer from a revoked investor", async () => {
    const { agent, mint, config } = await setupMintWithConfig();
    const alice = anchor.web3.Keypair.generate();
    const bob = anchor.web3.Keypair.generate();

    const aliceAta = await kycAndThaw(agent, mint.publicKey, config, alice);
    const bobAta = await kycAndThaw(agent, mint.publicKey, config, bob);
    await mintTo(provider.connection, payer, mint.publicKey, aliceAta.address, payer, 1_000 * 10 ** DECIMALS, [], undefined, TOKEN_2022_PROGRAM_ID);

    await compliance.methods
      .revokeInvestor()
      .accounts({
        agent: agent.publicKey,
        config,
        investorRecord: investorPda(compliance.programId, mint.publicKey, alice.publicKey)[0],
      })
      .signers([agent])
      .rpc();

    const result = await attemptTransfer(mint.publicKey, aliceAta.address, bobAta.address, alice, 10 * 10 ** DECIMALS);
    expect(result.ok).to.equal(false, "transfer from a revoked investor must fail");
  });
});
