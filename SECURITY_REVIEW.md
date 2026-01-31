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
| 5 | MEDIUM | No upgrade authority governance | 📝 DOCUMENTED | Inline recommendations for Squads multisig, timelock, role separation. Requires operational decision. |
| 6 | MEDIUM | Cannot update bid price | ✅ FIXED | Added `update_bid` instruction — users can replace `hash_commitment` and `max_fdv_encrypted` during the commitment window. |
| 7 | LOW | `reveal_count` boundary | ⚠️ ACCEPTED | Mitigated by `bid_revealed` flag. No code change needed. |

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

### 5. No upgrade authority governance 📝 DOCUMENTED

**Severity**: MEDIUM

A single authority key controls all admin operations: initialization, funding, settlement proposal, pausing, cancellation, and refund mode. There is no multisig requirement or timelock on any critical operation.

**Recommendation** (documented in `initialize_sale.rs`): Use a Squads multisig as the authority, add timelocks for critical operations, and consider separating roles (proposer vs admin vs emergency). This is an operational decision rather than a code fix.

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

### Remaining

6. **Add upgrade authority governance**. Use a multisig (e.g., Squads) for the authority key, or add a timelock for critical operations. (MEDIUM — operational decision)
7. **Add drand liveness fallback**. If the drand round hasn't been submitted within a timeout, allow fallback reveal or cancellation. (MEDIUM)
8. **Add event indexing** for off-chain monitoring. (LOW)
9. **Consider a dispute/challenge period** before finalization. (LOW)
10. **Add per-user maximum bid amount** (currently only per-sale `raise_max`). (LOW)

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
