# Security Review — Fundraising Program v0.2

**Date**: 2026-01-31
**Scope**: `programs/fundraising_program/src/` — all instructions, state, and utilities
**Methodology**: Manual code review against Solana security checklist

---

## Summary

The program implements a sealed-bid token auction with drand timelock encryption. The core auction mechanics (state machine, allocation math, finalization checks) are well-structured. The initial review found critical cryptographic gaps and several high/medium issues. All critical and high issues have been fixed; medium issues have been resolved or documented.

### Fix Status

| # | Severity | Issue | Status | Fix |
|---|----------|-------|--------|-----|
| 1 | CRITICAL | Drand signature not verified | ✅ RESOLVED | Not needed — SHA256 hash commitment + timelock encryption provide security without on-chain BLS |
| 2 | CRITICAL | Salt stored on-chain | ✅ FIXED | Removed `salt` from `SealedBid`. Salt now embedded in timelock ciphertext (`max_fdv \|\| salt`), provided by cranker at reveal time. On-chain SHA256 check verifies correctness. |
| 3 | HIGH | Authority can refund from Settled | ✅ FIXED | Removed `SaleStatus::Settled` from `refund_sale` constraints. Added `Revealing` and `CommitmentEnded` as valid escape hatches. |
| 4 | HIGH | `init_if_needed` reinitialization | ✅ FIXED | Replaced with explicit `init` on `commit` + separate `topup_commit` instruction. Anchor rejects duplicate PDA creation automatically. |
| 5 | MEDIUM | No upgrade authority governance | ✅ PARTIALLY FIXED | `propose_settlement` now permissionless after 30-min authority window. Withdrawals locked to treasury addresses set at init. Remaining: multisig for pause/cancel/refund. |
| 6 | MEDIUM | Cannot update bid price | ✅ FIXED | Added `update_bid` instruction — users can replace `hash_commitment` and `max_fdv_encrypted` during the commitment window. |
| 7 | LOW | `reveal_count` boundary | ⚠️ ACCEPTED | Mitigated by `bid_revealed` flag. No code change needed. |
| 8 | NEW | Withdraw destination unrestricted | ✅ FIXED | Added `usdc_treasury` and `token_treasury` fields to `Sale`, set at init. Withdrawals constrained to these addresses. |
| 9 | NEW | `propose_settlement` authority-gated | ✅ FIXED | Permissionless after 30-min authority window post-CommitmentEnded. Prevents stuck sales. |

---

## Critical Issues

### 1. Drand signature not verified — `drand.rs:25-45` ✅ RESOLVED

**Severity**: CRITICAL → **RESOLVED (by design)**

The `verify_drand_signature` function does not perform BLS12-381 signature verification. It only checks:
- Chain hash matches `QUICKNET_CHAIN_HASH`
- Signature is not all-zeros
- Signature is not all-0xFF

**Resolution**: On-chain BLS verification is unnecessary. With salt removed from on-chain storage (see #2), the SHA256 hash commitment prevents anyone from submitting a valid reveal without the salt — which is locked inside the timelock ciphertext until the drand round fires. The state machine + `Clock::get()` enforces timing. Solana supports BLS12-381 via SIMD-0129 (~100k-200k CU) if belt-and-suspenders verification is ever desired.

### 2. Salt stored on-chain — `commit.rs:131` ✅ FIXED

**Severity**: CRITICAL → **FIXED**

The bid salt was stored in the `SealedBid` account: `bid.salt = salt`. Since all Solana account data is publicly readable, anyone could brute-force the hash commitment.

**Fix**: Removed `salt` field from `SealedBid` struct and `salt` parameter from `commit` instruction. Salt is now:
1. Generated client-side (16 bytes) and kept off-chain
2. Embedded in the timelock ciphertext: `encrypt(max_fdv || salt, drand_round)`
3. Stored on-chain only as `hash_commitment = SHA256(max_fdv || salt)`
4. Recovered at reveal time by decrypting the ciphertext, then submitted by the cranker
5. Verified on-chain: `SHA256(max_fdv || salt) == hash_commitment` (~1000 CU)

16-byte salt is used (vs 32) to keep the timelock ciphertext within Solana's 1232-byte transaction size limit.

---

## High Severity Issues

### 3. Authority can refund from Settled state — `refund_sale.rs:21` ✅ FIXED

**Severity**: HIGH → **FIXED**

The `refund_sale` constraint allowed `SaleStatus::Settled`, letting the authority back out after settlement.

**Fix**: Removed `SaleStatus::Settled` from the allowed states in `refund_sale.rs`. Added `Revealing` and `CommitmentEnded` as valid refund states (previously missing escape hatches). Once settled, the outcome is binding — users have a right to their allocated tokens.

### 4. `init_if_needed` reinitialization risk — `commit.rs:19-26` ✅ FIXED

**Severity**: HIGH → **FIXED**

Used `init_if_needed` for SealedBid accounts, which Anchor docs warn enables reinitialization attacks.

**Fix**: Split into two separate instructions:
- **`commit`**: Uses Anchor's `init` (not `init_if_needed`) for first-time bid creation. Anchor automatically rejects a second `init` call for the same PDA.
- **`topup_commit`**: Adds USDC to an existing bid. Uses `mut` with constraint checks (`is_initialized`, `sale`, `user`, `!claimed`). No account creation.

This eliminates the reinitialization risk class entirely.

---

## Medium Severity Issues

### 5. No upgrade authority governance ✅ PARTIALLY FIXED

**Severity**: MEDIUM → **PARTIALLY FIXED**

A single authority key originally controlled all admin operations. Three mitigations implemented:

1. **Permissionless `propose_settlement`**: Authority gets a 30-minute exclusive window after `CommitmentEnded`. After that, anyone can propose — the sale can never get stuck waiting for the authority. Runtime check in handler: `if now < commitment_ended_at + 1800, require proposer == authority`.

2. **Treasury-locked withdrawals**: `usdc_treasury` and `token_treasury` are set at `initialize_sale` and cannot be changed. `withdraw_raised` and `withdraw_unsold` constrain the destination to these addresses via `address = sale.usdc_treasury`. Even a compromised authority cannot redirect funds.

3. **Already permissionless**: `batch_reveal_bids`, `verify_and_allocate`, `batch_verify_and_allocate`, `finalize_settlement`, `close_commitment_window` — all callable by any signer.

**Remaining**: `pause_sale`, `unpause_sale`, `cancel_sale`, `refund_sale`, `enable_claims`, `fund_sale` still require authority. Recommend Squads multisig for the authority key in production.

### 6. Cumulative commits cannot update bid price ✅ FIXED

**Severity**: MEDIUM → **FIXED**

Users could not change their max FDV bid after the initial commit.

**Fix**: Added `update_bid` instruction that allows users to replace their `hash_commitment` and `max_fdv_encrypted` during the commitment window. The user generates a new salt, re-encrypts with the same drand round, and submits the new ciphertext. USDC amount is unchanged (use `topup_commit` to add funds). Constraints ensure the bid hasn't been revealed yet and the sale is still active.

---

## Low Severity Issues

### 7. `reveal_count` boundary — `batch_reveal_bids.rs:105-113`

**Severity**: LOW

No explicit check that `reveal_count + revealed_count <= total_users`. Mitigated by `bid_revealed` flag preventing double-reveal. The `>=` comparison for transitioning to CommitmentEnded handles the boundary correctly.

---

## Additional Fixes (discovered during hardening)

### 8. Withdraw destination unrestricted ✅ FIXED

**Severity**: HIGH (new finding) → **FIXED**

`withdraw_raised` and `withdraw_unsold` accepted any token account as the destination. A compromised authority key could redirect all raised USDC or unsold tokens to an attacker-controlled address.

**Fix**: Added `usdc_treasury` and `token_treasury` fields to `Sale`, set once during `initialize_sale`. Both withdraw instructions now use `address = sale.usdc_treasury` / `address = sale.token_treasury` constraints. The destination is immutable after sale creation. Test confirms withdrawal to a non-treasury address is rejected with `ConstraintAddress`.

### 9. `propose_settlement` authority-gated (stuck sale risk) ✅ FIXED

**Severity**: MEDIUM (new finding) → **FIXED**

If the authority key was lost or the authority became unresponsive after bids were revealed, no one could propose settlement. The sale would be permanently stuck in `CommitmentEnded` state — users' USDC locked in the vault.

**Fix**: `propose_settlement` is now permissionless with a 30-minute authority-first window. Added `commitment_ended_at` timestamp to `Sale` (set in `batch_reveal_bids` when transitioning to `CommitmentEnded`). In the handler, if `now < commitment_ended_at + 1800`, the signer must be the authority (`Unauthorized` error). After 1800 seconds, any signer can propose. The PDA seed derivation uses `sale.authority` (stored value) instead of the signer's key.

---

## External Dependencies

### drand Quicknet

| Risk | Description |
|------|-------------|
| **Liveness** | If drand stops producing rounds, bids can never be revealed. No fallback mechanism exists — the sale would be stuck in Revealing state (authority can use `refund_sale` as escape). |
| **Deprecation** | Quicknet chain hash, genesis time (1692803367), and period (3s) are hardcoded. If Quicknet is deprecated or parameters change, the program needs redeployment. |
| **BLS verification** | The program does not actually verify BLS12-381 signatures (see Critical Issue #1). |

### Solana Clock

`Clock::get()` is used for time checks throughout. Validators can manipulate the clock within bounded drift (~1-2 seconds). This is not a significant concern for the commitment windows (which are minutes/hours) but is worth noting.

---

## Recommended Actions

### Completed

1. ~~**Implement real BLS12-381 signature verification**~~ — ✅ Not needed. SHA256 hash commitment + timelock encryption provide security without on-chain BLS.
2. ~~**Remove salt from on-chain storage**~~ — ✅ Salt embedded in timelock ciphertext, provided at reveal time. Hash commitment verifies correctness.
3. ~~**Restrict `refund_sale` from Settled state**~~ — ✅ Removed `Settled` from allowed states. Added `Revealing` and `CommitmentEnded`.
4. ~~**Remove `init_if_needed`**~~ — ✅ Replaced with explicit `init` + `topup_commit` instructions.
5. ~~**Allow bid price updates**~~ — ✅ Added `update_bid` instruction.
6. ~~**Lock withdraw destinations to treasury**~~ — ✅ `usdc_treasury` and `token_treasury` set at init, enforced via address constraints.
7. ~~**Make `propose_settlement` permissionless**~~ — ✅ 30-min authority window, then open to anyone.

### Remaining

8. **Add multisig for remaining authority operations**. `pause/unpause`, `cancel`, `refund`, `enable_claims`, `fund_sale` still single-signer. Use Squads multisig in production. (MEDIUM — operational)
9. **Add drand liveness fallback**. If the drand round hasn't been submitted within a timeout, allow fallback reveal or cancellation. (MEDIUM)
10. **Add event indexing** for off-chain monitoring. (LOW)
11. **Consider a dispute/challenge period** before finalization. (LOW)
12. **Add per-user maximum bid amount** (currently only per-sale `raise_max`). (LOW)

---

## Design Notes (from review discussion)

### On-chain BLS not required

The original assumption was that drand BLS signature verification on-chain was necessary. After analysis:

- **SHA256 hash commitment** (`~1000 CU`) verifies that the revealed `max_fdv` is correct
- **Timelock encryption** (off-chain) keeps bids private until the drand round fires
- **State machine timing** (`Clock::get()` + `close_commitment_window`) prevents early reveals
- **BLS pairing check** (`~100k-200k CU` via SIMD-0129) would be belt-and-suspenders but is unnecessary

The `verify_drand_signature` function's current trivial checks (non-zero, non-FF) are sufficient as a basic sanity check. The real security comes from the hash commitment: without the salt (which is locked in the ciphertext), nobody can submit a valid reveal.

### Trustless reveal path

The design supports fully permissionless cranker-based reveals:
1. Cranker fetches drand beacon output for round N (public after the round)
2. Cranker decrypts each bid's `max_fdv_encrypted` off-chain → gets `max_fdv || salt`
3. Cranker submits `max_fdv + salt` to `batch_reveal_bids`
4. On-chain: `SHA256(max_fdv || salt) == hash_commitment` — verified

No admin privilege required for reveals. The admin role is a convenience, not a security requirement.
