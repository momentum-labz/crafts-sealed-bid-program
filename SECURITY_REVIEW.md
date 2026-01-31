# Security Review — Fundraising Program v0.2

**Date**: 2026-01-31
**Scope**: `programs/fundraising_program/src/` — all instructions, state, and utilities
**Methodology**: Manual code review against Solana security checklist

---

## Summary

The program implements a sealed-bid token auction with drand timelock encryption. The core auction mechanics (state machine, allocation math, finalization checks) are well-structured. However, the cryptographic foundation has critical gaps: the drand BLS signature is not actually verified, and the hash commitment salt is stored on-chain, which together mean bids are not sealed in practice.

---

## Critical Issues

### 1. Drand signature not verified — `drand.rs:25-45`

**Severity**: CRITICAL

The `verify_drand_signature` function does not perform BLS12-381 signature verification. It only checks:
- Chain hash matches `QUICKNET_CHAIN_HASH`
- Signature is not all-zeros
- Signature is not all-0xFF

Any 48 bytes that aren't trivially zero/FF will pass. This means anyone can call `batch_reveal_bids` with a fabricated signature.

**Impact**: The drand timelock property is completely bypassed. Bids can be revealed at any time during the Revealing phase without the actual drand beacon output.

### 2. Salt stored on-chain — `commit.rs:131`

**Severity**: CRITICAL

The bid salt is stored in the `SealedBid` account: `bid.salt = salt`. Since all Solana account data is publicly readable, anyone can:

1. Read the salt from the SealedBid account
2. Iterate over all `max_fdv` values in `[fdv_min, fdv_max]`
3. Compute `SHA256(max_fdv || salt)` for each
4. Match against `hash_commitment`

The hash commitment scheme provides **zero privacy** when the salt is public.

**Combined impact of issues 1 + 2**: Bids are effectively public during the commitment phase. Any observer can determine every bidder's max FDV in real-time, defeating the sealed-bid property entirely.

---

## High Severity Issues

### 3. Authority can refund from Settled state — `refund_sale.rs:21`

**Severity**: HIGH

The `refund_sale` constraint allows `SaleStatus::Settled`. After all bids are verified and settlement is finalized, the authority can still trigger refund mode. Users who expected tokens at the clearing price would only receive their USDC back.

**Impact**: Authority can back out of an unfavorable sale after seeing results. This undermines the binding nature of the auction.

### 4. `init_if_needed` reinitialization risk — `commit.rs:19-26`

**Severity**: HIGH (mitigated to MEDIUM in practice)

Uses `init_if_needed` for SealedBid accounts. While mitigated by the `is_initialized` flag check and unique PDA derivation `[b"sealed_bid", sale, user]`, Anchor documentation warns against this pattern. The mitigation appears sufficient but the pattern is inherently risky.

---

## Medium Severity Issues

### 5. No upgrade authority governance

**Severity**: MEDIUM

A single authority key controls all admin operations: initialization, funding, settlement proposal, pausing, cancellation, and refund mode. There is no multisig requirement or timelock on any critical operation.

### 6. Cumulative commits cannot update bid price — `commit.rs:154-163`

**Severity**: MEDIUM

On subsequent commits (top-ups), the `hash_commitment`, `salt`, and `max_fdv_encrypted` are NOT updated — only the USDC amount increases. If a user wants to change their max FDV bid, they cannot. This is a UX limitation that could lead to suboptimal bidding.

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

### Priority 1 — Critical (RESOLVED)

1. ~~**Implement real BLS12-381 signature verification**~~ — **Not needed on-chain.** Solana supports BLS12-381 via SIMD-0129 (pairing, G1/G2 ops, ~100k-200k CU per check), but in our design the SHA256 hash commitment already verifies correctness of revealed bids. The drand timelock encryption provides off-chain privacy; the state machine + `Clock::get()` provides timing enforcement. On-chain BLS verification would only be needed if we wanted to remove clock-based timing entirely.

2. **Remove salt from on-chain storage** ✅ **IMPLEMENTING** — Embed `max_fdv || salt` in the timelock ciphertext (`max_fdv_encrypted`). Remove `salt` field from `SealedBid` and from `commit` instruction parameters. At reveal time, the cranker/admin decrypts the ciphertext off-chain to obtain both `max_fdv` and `salt`, then submits both. On-chain SHA256 check verifies `SHA256(max_fdv || salt) == hash_commitment`. Salt never stored on-chain, hash commitment still works.

   **Why this works trustlessly**: Submitting a valid `(max_fdv, salt)` pair that passes the hash check is itself proof of decryption, because the salt only exists inside the timelock ciphertext. Before the drand round, nobody can extract the salt. After it, anyone can decrypt and reveal — no admin required.

### Priority 2 — High

3. **Restrict `refund_sale` from Settled state**. Either remove `SaleStatus::Settled` from the allowed states, or require a timelock/multisig for post-settlement refunds.

4. **Add upgrade authority governance**. Use a multisig (e.g., Squads) for the authority key, or add a timelock for critical operations like cancellation and refund.

### Priority 3 — Medium

5. **Add drand liveness fallback**. If the drand round hasn't been submitted within a timeout after `commitment_end`, allow the authority (or a governance mechanism) to trigger a fallback reveal or cancellation.

6. **Consider removing `init_if_needed`**. Use an explicit `init_bid` instruction followed by a separate `top_up` instruction. This eliminates the reinitialization risk class entirely.

### Priority 4 — Improvements

7. **Add event indexing** for off-chain monitoring (events are partially implemented already).
8. **Consider a dispute/challenge period** before finalization, allowing bidders to contest the proposed settlement.
9. **Add per-user maximum bid amount** (currently only per-sale `raise_max` is checked against cumulative bid amount).
10. **Allow bid price updates** — let users update their `hash_commitment` and `max_fdv_encrypted` during the commitment window (with the same or higher USDC amount).

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
