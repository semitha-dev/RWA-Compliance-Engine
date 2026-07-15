# Architecture

## Account model

### `rwa-compliance::Config` (PDA: `["config", mint]`)

```rust
pub struct Config {
    pub authority: Pubkey,           // admin: pause, jurisdictions, force_transfer, authority transfer
    pub pending_authority: Pubkey,   // set during a 2-step authority transfer; default() otherwise
    pub agent: Pubkey,               // day-to-day compliance ops: upsert/revoke investor, freeze/thaw
    pub mint: Pubkey,                // bound at init; validated against on every downstream instruction
    pub paused: bool,
    pub lockup_until: i64,           // mint-wide issuance lockup
    pub max_holders: u32,            // 0 = unlimited
    pub holder_count: u32,
    pub allowed_jurisdictions: [u8; 32], // 256-bit bitmap, indexed by InvestorRecord.country_code
    pub bump: u8,
}
```

This PDA is also the mint's **Permanent Delegate** and **Freeze Authority** (set at mint
creation time, outside this program — see `tests/utils.ts::createComplianceMint`). That's
what lets `force_transfer` and `thaw_investor_account` / `freeze_investor_account` sign
CPIs into Token-2022 without a human-held keypair ever being the thing that can move
investor funds unilaterally.

Splitting `authority` from `agent` mirrors how issuers actually operate: a compliance
operations team runs KYC intake and day-to-day freezes (`agent`), while pausing the
entire mint or executing a clawback needs a higher bar (`authority`).

### `rwa-compliance::InvestorRecord` (PDA: `["investor", mint, owner]`)

```rust
pub struct InvestorRecord {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub country_code: u8,       // indexes Config.allowed_jurisdictions
    pub kyc_expiry: i64,        // 0 = never expires
    pub lockup_until: i64,      // per-investor lockup (e.g. vesting cliff), independent of mint-wide lockup
    pub max_balance: u64,       // 0 = unlimited
    pub status: u8,             // bitflags: ALLOWLISTED | ACCREDITED | FROZEN | REVOKED
    pub bump: u8,
}
```

One record per (mint, owner) pair — a wallet holding two different RWA tokens under this
same compliance program has two independent records, which is correct: KYC status,
jurisdiction, and lockup terms are frequently offering-specific, not identity-global.

## Instruction inventory

**`rwa-compliance`** (control plane, all state-mutating):

| Instruction | Signer | Effect |
|---|---|---|
| `initialize_config` | payer (becomes authority) | creates `Config` for a mint |
| `manage_jurisdictions` | authority | flips a bit in the jurisdiction bitmap |
| `upsert_investor` | agent | create/update an `InvestorRecord` |
| `revoke_investor` | agent | sets REVOKED, clears ALLOWLISTED |
| `thaw_investor_account` | agent | CPI: Token-2022 `ThawAccount`, gated on valid `InvestorRecord` |
| `freeze_investor_account` | agent | CPI: Token-2022 `FreezeAccount`, independent of KYC status |
| `force_transfer` | authority | CPI: Token-2022 `TransferChecked` signed by the Config PDA as Permanent Delegate |
| `set_paused` | authority | emergency stop |
| `initiate_authority_transfer` / `accept_authority_transfer` | authority / nominee | 2-step handover |

**`rwa-transfer-hook`** (enforcement, read-only):

| Instruction | Invoked by | Effect |
|---|---|---|
| `initialize_extra_account_meta_list` | anyone (one-time, per mint) | writes the TLV-encoded list of extra accounts Token-2022 must resolve on every transfer |
| `execute` | Token-2022, via CPI, on every transfer | runs the enforcement chain; never callable usefully outside a live transfer |

## How Token-2022 resolves the hook's extra accounts

A `TransferChecked` instruction only carries 4 accounts by default (source, mint,
destination, owner). The hook needs three more per transfer: the mint's `Config`, and
both parties' `InvestorRecord`. `ExtraAccountMetaList` (a PDA at
`["extra-account-metas", mint]`, owned by the hook program) tells Token-2022's client-side
helper (`createTransferCheckedWithTransferHookInstruction`) how to derive them:

```
extra_account_metas() =
  [0] compliance_program            -- fixed pubkey, so later entries can reference it as an external-PDA owner
  [1] Config PDA                    = ["config", mint]                                          owned by compliance_program
  [2] source InvestorRecord PDA     = ["investor", mint, source_token.data[32..64]]              owned by compliance_program
  [3] destination InvestorRecord PDA= ["investor", mint, destination_token.data[32..64]]         owned by compliance_program
```

Entries `[2]` and `[3]` use `Seed::AccountData { account_index, data_index: 32, length: 32 }`
— i.e. "read 32 bytes starting at offset 32 of account N in the transfer" — rather than
`Seed::AccountKey` pointing at a separately supplied account. Offset 32 is where a
Token-2022 base `Account`'s `owner: Pubkey` field lives (`mint` occupies bytes 0..32,
`owner` occupies 32..64). The effect: the destination `InvestorRecord` PDA the hook reads
is *mathematically pinned* to the actual owner of the actual destination token account
in *this* transfer — a client cannot supply a different (more permissive) record and
have the hook use it instead, because the PDA derivation itself reads the real value out
of the account being credited.

## Why two programs instead of one

Token-2022 CPIs into the transfer hook with every account marked read-only — a hook
program is architecturally incapable of mutating anything mid-transfer, by Solana's
design, not by choice. Given that constraint, folding freeze/thaw/clawback into the same
program as `execute` would only obscure the boundary that already exists in the runtime.
Keeping them separate makes the read-only contract explicit in the code structure itself:
if you're editing `rwa-transfer-hook`, you cannot accidentally write state, because there
is no state-owning code in that crate to reach for.

## Compliance flow, end to end

```
issuer creates Token-2022 mint
  extensions: DefaultAccountState=Frozen, PermanentDelegate=ConfigPDA, TransferHook=hook_program
       │
       ▼
rwa_compliance::initialize_config(agent, lockup_until, max_holders)
       │
       ▼
rwa_transfer_hook::initialize_extra_account_meta_list()   -- one-time per mint
       │
       ▼
agent: rwa_compliance::upsert_investor(owner, country_code, kyc_expiry, lockup, max_balance)
       │
       ▼
agent: rwa_compliance::thaw_investor_account()   -- lifts the default-frozen state
       │                                             (fails if InvestorRecord isn't ALLOWLISTED)
       ▼
holder: createTransferCheckedWithTransferHookInstruction(...)
       │
       ▼
Token-2022: validates TransferChecked, sets `transferring=true` on both accounts,
            CPIs into rwa_transfer_hook::execute with all accounts read-only
       │
       ▼
execute(): is_transferring? -> mint match? -> paused? -> lockup? -> allowlist/KYC/frozen/
           revoked (both sides)? -> jurisdiction? -> balance cap? -> emit event -> Ok(())
       │
       ▼
Token-2022: completes the transfer, clears `transferring` on both accounts
```

If any check in `execute` fails, the entire transaction is rolled back — including the
token balance changes Token-2022 was about to commit. There is no partial-transfer state.
