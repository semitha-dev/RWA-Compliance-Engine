import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import { RwaCompliance } from "../target/types/rwa_compliance";
import { RwaTransferHook } from "../target/types/rwa_transfer_hook";
import { createComplianceMint, investorPda, configPda, ONE_DAY, now } from "./utils";

describe("04: compliance admin — authority/agent permission boundaries", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const compliance = anchor.workspace.RwaCompliance as Program<RwaCompliance>;
  const hook = anchor.workspace.RwaTransferHook as Program<RwaTransferHook>;
  const payer = (provider.wallet as anchor.Wallet).payer;

  async function setupMint() {
    const agent = anchor.web3.Keypair.generate();
    const { mint } = await createComplianceMint(provider.connection, payer, compliance.programId, hook.programId);
    const [config] = configPda(compliance.programId, mint.publicKey);
    await compliance.methods
      .initializeConfig(agent.publicKey, new anchor.BN(0), 0)
      .accounts({ authority: payer.publicKey, mint: mint.publicKey })
      .rpc();
    return { agent, mint, config };
  }

  it("rejects a non-agent trying to upsert an investor", async () => {
    const { mint, config } = await setupMint();
    const impostor = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(impostor.publicKey, 1e9).then((sig) =>
      provider.connection.confirmTransaction(sig, "confirmed")
    );
    const someInvestor = anchor.web3.Keypair.generate();

    let threw = false;
    try {
      await compliance.methods
        .upsertInvestor(someInvestor.publicKey, 1, new anchor.BN(now() + ONE_DAY), new anchor.BN(0), new anchor.BN(0), false)
        .accounts({ agent: impostor.publicKey, config })
        .signers([impostor])
        .rpc();
    } catch (_e) {
      threw = true;
    }
    expect(threw).to.equal(true, "a non-agent must not be able to upsert an investor record");
  });

  it("rejects a non-authority trying to pause the mint", async () => {
    const { config } = await setupMint();
    const impostor = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(impostor.publicKey, 1e9).then((sig) =>
      provider.connection.confirmTransaction(sig, "confirmed")
    );

    let threw = false;
    try {
      await compliance.methods.setPaused(true).accounts({ authority: impostor.publicKey, config }).signers([impostor]).rpc();
    } catch (_e) {
      threw = true;
    }
    expect(threw).to.equal(true, "a non-authority must not be able to pause the mint");
  });

  it("rejects a non-agent trying to thaw an investor's token account", async () => {
    const { agent, mint, config } = await setupMint();
    const investor = anchor.web3.Keypair.generate();
    const impostor = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(impostor.publicKey, 1e9).then((sig) =>
      provider.connection.confirmTransaction(sig, "confirmed")
    );

    await compliance.methods
      .upsertInvestor(investor.publicKey, 1, new anchor.BN(now() + ONE_DAY), new anchor.BN(0), new anchor.BN(0), false)
      .accounts({ agent: agent.publicKey, config })
      .signers([agent])
      .rpc();

    const { getOrCreateAssociatedTokenAccount, TOKEN_2022_PROGRAM_ID } = await import("@solana/spl-token");
    const ata = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mint.publicKey,
      investor.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    let threw = false;
    try {
      await compliance.methods
        .thawInvestorAccount()
        .accounts({
          agent: impostor.publicKey,
          config,
          investorRecord: investorPda(compliance.programId, mint.publicKey, investor.publicKey)[0],
          mint: mint.publicKey,
          tokenAccount: ata.address,
        })
        .signers([impostor])
        .rpc();
    } catch (_e) {
      threw = true;
    }
    expect(threw).to.equal(true, "a non-agent must not be able to thaw a token account");
  });

  it("completes a full two-step authority transfer", async () => {
    const { config } = await setupMint();
    const newAuthority = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(newAuthority.publicKey, 1e9).then((sig) =>
      provider.connection.confirmTransaction(sig, "confirmed")
    );

    await compliance.methods
      .initiateAuthorityTransfer(newAuthority.publicKey)
      .accounts({ authority: payer.publicKey, config })
      .rpc();

    let midway = await compliance.account.config.fetch(config);
    expect(midway.authority.toBase58()).to.equal(payer.publicKey.toBase58(), "authority must not change until accepted");
    expect(midway.pendingAuthority.toBase58()).to.equal(newAuthority.publicKey.toBase58());

    await compliance.methods
      .acceptAuthorityTransfer()
      .accounts({ pendingAuthority: newAuthority.publicKey, config })
      .signers([newAuthority])
      .rpc();

    const after = await compliance.account.config.fetch(config);
    expect(after.authority.toBase58()).to.equal(newAuthority.publicKey.toBase58());
    expect(after.pendingAuthority.toBase58()).to.equal(anchor.web3.PublicKey.default.toBase58());
  });

  it("rejects an arbitrary account trying to accept an authority transfer it wasn't nominated for", async () => {
    const { config } = await setupMint();
    const nominee = anchor.web3.Keypair.generate();
    const impostor = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(impostor.publicKey, 1e9).then((sig) =>
      provider.connection.confirmTransaction(sig, "confirmed")
    );

    await compliance.methods
      .initiateAuthorityTransfer(nominee.publicKey)
      .accounts({ authority: payer.publicKey, config })
      .rpc();

    let threw = false;
    try {
      await compliance.methods
        .acceptAuthorityTransfer()
        .accounts({ pendingAuthority: impostor.publicKey, config })
        .signers([impostor])
        .rpc();
    } catch (_e) {
      threw = true;
    }
    expect(threw).to.equal(true, "only the nominated pending authority may accept the transfer");
  });
});
