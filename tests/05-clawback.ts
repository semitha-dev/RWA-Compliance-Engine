import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  TOKEN_2022_PROGRAM_ID,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { RwaCompliance } from "../target/types/rwa_compliance";
import { RwaTransferHook } from "../target/types/rwa_transfer_hook";
import { createComplianceMint, investorPda, configPda, DECIMALS, ONE_DAY, now } from "./utils";

describe("05: clawback and freeze — regulatory override paths", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const compliance = anchor.workspace.RwaCompliance as Program<RwaCompliance>;
  const hook = anchor.workspace.RwaTransferHook as Program<RwaTransferHook>;
  const payer = (provider.wallet as anchor.Wallet).payer;

  async function setupInvestorWithBalance(config: anchor.web3.PublicKey, agent: anchor.web3.Keypair, mint: anchor.web3.PublicKey, investor: anchor.web3.Keypair, amount: number) {
    await compliance.methods
      .upsertInvestor(investor.publicKey, 1, new anchor.BN(now() + 365 * ONE_DAY), new anchor.BN(0), new anchor.BN(0), false)
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

    if (amount > 0) {
      await mintTo(provider.connection, payer, mint, ata.address, payer, amount, [], undefined, TOKEN_2022_PROGRAM_ID);
    }

    return ata;
  }

  it("force_transfer moves tokens via the Permanent Delegate without the holder's signature", async () => {
    const agent = anchor.web3.Keypair.generate();
    const { mint } = await createComplianceMint(provider.connection, payer, compliance.programId, hook.programId);
    const [config] = configPda(compliance.programId, mint.publicKey);

    await compliance.methods
      .initializeConfig(agent.publicKey, new anchor.BN(0), 0)
      .accounts({ authority: payer.publicKey, mint: mint.publicKey })
      .rpc();
    await compliance.methods.manageJurisdictions(1, true).accounts({ authority: payer.publicKey, config }).rpc();
    await hook.methods
      .initializeExtraAccountMetaList()
      .accounts({ payer: payer.publicKey, mint: mint.publicKey, complianceProgram: compliance.programId })
      .rpc();

    const compromised = anchor.web3.Keypair.generate();
    const recovery = anchor.web3.Keypair.generate();

    const amount = 500 * 10 ** DECIMALS;
    const compromisedAta = await setupInvestorWithBalance(config, agent, mint.publicKey, compromised, amount);
    const recoveryAta = await setupInvestorWithBalance(config, agent, mint.publicKey, recovery, 0);

    // Note: no signature from `compromised` is provided here at all -- only the
    // config authority signs. This is the entire point of a Permanent Delegate.
    await compliance.methods
      .forceTransfer(new anchor.BN(amount), 1)
      .accounts({
        authority: payer.publicKey,
        config,
        mint: mint.publicKey,
        from: compromisedAta.address,
        to: recoveryAta.address,
      })
      .rpc();

    const recoveryAccount = await getAccount(provider.connection, recoveryAta.address, "confirmed", TOKEN_2022_PROGRAM_ID);
    expect(recoveryAccount.amount.toString()).to.equal(BigInt(amount).toString());

    const compromisedAccount = await getAccount(provider.connection, compromisedAta.address, "confirmed", TOKEN_2022_PROGRAM_ID);
    expect(compromisedAccount.amount.toString()).to.equal("0");
  });

  it("force_transfer succeeds even while the source account is administratively frozen", async () => {
    const agent = anchor.web3.Keypair.generate();
    const { mint } = await createComplianceMint(provider.connection, payer, compliance.programId, hook.programId);
    const [config] = configPda(compliance.programId, mint.publicKey);

    await compliance.methods
      .initializeConfig(agent.publicKey, new anchor.BN(0), 0)
      .accounts({ authority: payer.publicKey, mint: mint.publicKey })
      .rpc();
    await compliance.methods.manageJurisdictions(1, true).accounts({ authority: payer.publicKey, config }).rpc();
    await hook.methods
      .initializeExtraAccountMetaList()
      .accounts({ payer: payer.publicKey, mint: mint.publicKey, complianceProgram: compliance.programId })
      .rpc();

    const seized = anchor.web3.Keypair.generate();
    const treasury = anchor.web3.Keypair.generate();

    const amount = 250 * 10 ** DECIMALS;
    const seizedAta = await setupInvestorWithBalance(config, agent, mint.publicKey, seized, amount);
    const treasuryAta = await setupInvestorWithBalance(config, agent, mint.publicKey, treasury, 0);

    await compliance.methods
      .freezeInvestorAccount()
      .accounts({ agent: agent.publicKey, config, mint: mint.publicKey, tokenAccount: seizedAta.address })
      .signers([agent])
      .rpc();

    // Permanent Delegate transfers bypass the frozen-account restriction by design --
    // this is the mechanism a court-ordered seizure relies on: freeze first (stop the
    // holder from moving funds), then claw back via the delegate at the issuer's pace.
    await compliance.methods
      .forceTransfer(new anchor.BN(amount), 2)
      .accounts({ authority: payer.publicKey, config, mint: mint.publicKey, from: seizedAta.address, to: treasuryAta.address })
      .rpc();

    const treasuryAccount = await getAccount(provider.connection, treasuryAta.address, "confirmed", TOKEN_2022_PROGRAM_ID);
    expect(treasuryAccount.amount.toString()).to.equal(BigInt(amount).toString());
  });

  it("rejects a non-authority attempting a force_transfer", async () => {
    const agent = anchor.web3.Keypair.generate();
    const { mint } = await createComplianceMint(provider.connection, payer, compliance.programId, hook.programId);
    const [config] = configPda(compliance.programId, mint.publicKey);

    await compliance.methods
      .initializeConfig(agent.publicKey, new anchor.BN(0), 0)
      .accounts({ authority: payer.publicKey, mint: mint.publicKey })
      .rpc();
    await compliance.methods.manageJurisdictions(1, true).accounts({ authority: payer.publicKey, config }).rpc();
    await hook.methods
      .initializeExtraAccountMetaList()
      .accounts({ payer: payer.publicKey, mint: mint.publicKey, complianceProgram: compliance.programId })
      .rpc();

    const holder = anchor.web3.Keypair.generate();
    const destination = anchor.web3.Keypair.generate();
    const impostor = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(impostor.publicKey, 1e9).then((sig) =>
      provider.connection.confirmTransaction(sig, "confirmed")
    );

    const amount = 100 * 10 ** DECIMALS;
    const holderAta = await setupInvestorWithBalance(config, agent, mint.publicKey, holder, amount);
    const destinationAta = await setupInvestorWithBalance(config, agent, mint.publicKey, destination, 0);

    let threw = false;
    try {
      await compliance.methods
        .forceTransfer(new anchor.BN(amount), 1)
        .accounts({ authority: impostor.publicKey, config, mint: mint.publicKey, from: holderAta.address, to: destinationAta.address })
        .signers([impostor])
        .rpc();
    } catch (_e) {
      threw = true;
    }
    expect(threw).to.equal(true, "only the config authority may perform a force_transfer");
  });
});
