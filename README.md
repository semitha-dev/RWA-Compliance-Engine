# RWA Compliance Engine (Solana / Anchor / Token-2022)

[![CI](https://github.com/USERNAME/REPO/actions/workflows/ci.yml/badge.svg)](https://github.com/USERNAME/REPO/actions/workflows/ci.yml)

An on-chain compliance engine for tokenized real-world assets (RWAs): a Solana
Token-2022 security token that enforces KYC, jurisdiction restrictions, lockups, and
regulatory clawback **at the token level, on every transfer** — not as an off-chain
policy that a compliant client is trusted to respect.

## Why I built this

Every RWA tokenization platform (Ondo, Opening Bell, Paxos USDP) converges on the same
technical answer to "how do you make a security token that a securities lawyer will sign
off on": Token-2022's Transfer Hook, Permanent Delegate, and Default Account State
extensions, composed so that compliance is structural rather than advisory. I wanted to
prove I could build that composition correctly — including the parts of it that are
easy to get subtly wrong (see [Threat model](#threat-model) below) — rather than just
read about it.

## What this proves

- **It compiles and its tests pass in CI on every push** — see the badge above. I don't
  have a local Rust/Anchor toolchain, so GitHub Actions *is* the build: it installs
  Agave + Anchor from scratch and runs `anchor build` + `anchor test` (against a real
  `solana-test-validator`) on every commit. Nothing here is claimed that CI doesn't
  verify.
- **What it does *not* prove**: this has not been audited, and it has not been deployed
  to devnet or mainnet. Treat it as a demonstration of the architecture and the Rust,
  not as production-ready code.

## Architecture

Two programs, deliberately separated by responsibility:

| Program | Role |
|---|---|
| `rwa-compliance` | Registry + control plane. Owns the mint's Permanent Delegate and Freeze Authority (as a PDA). Handles KYC upsert/revoke, freeze/thaw, clawback, pause, jurisdiction rules, 2-step authority transfer. |
| `rwa-transfer-hook` | Token-2022 Transfer Hook. Read-only by construction — enforces the rules `rwa-compliance` owns, on every transfer, and rejects the entire transaction on any violation. |

```
Holder A ──TransferChecked──> Token-2022 ──CPI (read-only accounts)──> rwa-transfer-hook::execute
                                                                              │
                                                            reads Config + both InvestorRecords
                                                            (owned by rwa-compliance, resolved via
                                                             ExtraAccountMetaList)
                                                                              │
                                                                    reject or allow the whole tx
```

**The KYC gate is structural, not advisory.** The mint is created with
`DefaultAccountState = Frozen`, so *every* freshly created associated token account
starts unable to receive tokens at all — full stop, enforced by Token-2022 itself.
`rwa_compliance::thaw_investor_account` is the only path that lifts that freeze, and it
requires a valid, allowlisted `InvestorRecord` to exist first. There's no application
code path that lets a client "forget" to check KYC, because the token program won't let
the transfer happen regardless.

**Recipient-side compliance can't be spoofed by the client.** The hook needs each side's
`InvestorRecord` PDA, but rather than trust whatever accounts a client's transaction
supplies, `ExtraAccountMetaList` derives both PDAs directly from bytes 32..64 of the
source/destination token accounts' own on-chain data (the `owner` field) —
see [`initialize_extra_account_meta_list.rs`](programs/rwa-transfer-hook/src/instructions/initialize_extra_account_meta_list.rs).
A client cannot substitute a different, more-permissive `InvestorRecord` for the actual
recipient.

## Enforcement order (`execute`)

1. `is_transferring` flag valid on **both** the source and destination token accounts
2. Mint identity matches on both token accounts
3. Mint not paused
4. Mint-wide lockup expired
5. Source: allowlisted, not revoked, not frozen, KYC valid, investor-lockup expired
6. Destination: allowlisted, not revoked, not frozen, KYC valid
7. Destination jurisdiction permitted (256-bit bitmap on `Config`)
8. Destination balance cap not exceeded
9. Emit `TransferChecked` event

Any failure aborts the entire Solana transaction — Token-2022 CPIs are all-or-nothing.

## Threat model

This is the part I'd want a reviewer to actually read, because it's where "I followed a
tutorial" and "I understand the primitive" diverge:

- **Direct `Execute` invocation.** Token-2022 sets a `transferring` bit on the
  `TransferHookAccount` extension of both token accounts immediately before CPI-ing into
  the hook, and clears it right after. Without checking this flag, nothing stops a client
  from calling `execute` directly — bypassing Token-2022, and every check that "runs
  during a transfer," entirely. `execute.rs` checks it on **both** accounts; the
  [reference Solana example](https://github.com/solana-developers/program-examples/blob/main/tokens/token-2022/transfer-hook/whitelist/anchor/programs/transfer-hook/src/lib.rs)
  this space is usually learned from only checks the source, and uses `panic!()` rather
  than a typed error for its actual allowlist check.
- **Read-only account stripping.** Token-2022 converts every account in the transfer to
  read-only before CPI-ing into the hook, specifically so a hook program can never move
  funds or extend the sender's signing authority. `rwa-transfer-hook` is built around
  this constraint rather than working around it: it never attempts to write state, and
  all mutations (freeze/thaw/clawback) live in `rwa-compliance`, invoked directly by an
  agent/authority, never by the hook.
- **Forged-mint / cross-mint confusion.** `execute` explicitly checks that both the
  source and destination token accounts' `mint` field equals the `mint` account passed
  into the same instruction, closing off an attack where a malicious actor deploys their
  own mint (with this same hook attached) and tries to get `execute` to authorize a
  transfer using a different mint's compliance records.
- **Two-step authority transfer.** `initiate_authority_transfer` /
  `accept_authority_transfer` exist because a one-step transfer to a mistyped or
  unreachable key would permanently brick the `Config` PDA — there would be no signer
  left who could ever call an authority-gated instruction again.
- **Overflow safety.** `overflow-checks = true` is set in `[profile.release]` in the
  workspace `Cargo.toml`. Solana programs are normally built in release mode, where Rust
  silently wraps on overflow by default — a balance-cap check (`checked_add` guarded by
  `require!`) is only actually safe if the build profile agrees that overflow is an
  error, not silent wraparound.

## Repository layout

```
programs/
  rwa-compliance/       state (Config, InvestorRecord), errors, events, 9 instructions
  rwa-transfer-hook/     initialize_extra_account_meta_list, execute
tests/
  01-setup.ts                    Config + ExtraAccountMetaList initialization
  02-happy-path.ts                full KYC -> thaw -> mint -> transfer flow
  03-adversarial-transfers.ts     8 rejection paths (see below)
  04-compliance-admin.ts          agent/authority permission boundaries, 2-step transfer
  05-clawback.ts                  Permanent Delegate force_transfer, incl. while frozen
.github/workflows/ci.yml   installs Agave 2.1.21 + Anchor 0.31.1 from scratch, builds, tests
```

## Adversarial test coverage

Rejection paths exercised in `03-adversarial-transfers.ts` and `04-compliance-admin.ts`:
non-KYC'd recipient · expired KYC · direct `Execute` invocation (the `is_transferring`
bypass) · active global lockup · disallowed jurisdiction · balance cap exceeded ·
paused mint · revoked investor · non-agent calling agent-gated instructions ·
non-authority calling authority-gated instructions · accepting an authority transfer
you weren't nominated for.

## Key versions

Anchor `0.31.1` · Agave (CI) `2.1.21` · SPL interface crates `0.10.0` · Rust edition
2021. Agave is pinned to 2.1.21 in CI specifically because 3.x hard-requires `io_uring`,
which GitHub-hosted runners' kernels block — `solana-test-validator` panics instantly on
3.x here (tracked upstream: [anza-xyz/agave#10481](https://github.com/anza-xyz/agave/issues/10481)).
This only affects the CLI/validator used to run tests, not the on-chain program code or
the SPL dependency versions.

## Running locally

Requires Rust, the Agave CLI, and Anchor 0.31.1 installed (see `.github/workflows/ci.yml`
for exact install commands, or use `avm`):

```bash
anchor build
anchor test
```

## What I'd build next

- Devnet deployment with a live explorer link
- Confidential Transfers extension for balance privacy between compliant counterparties
- An indexer/API service surfacing `TransferChecked`/`InvestorUpserted` events for an
  issuer-facing compliance dashboard
- A CPI-level fuzz harness (e.g. via `trident`) targeting the `execute` enforcement chain
