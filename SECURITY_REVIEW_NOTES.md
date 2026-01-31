# Security Review — Detailed Notes

Running log of each instruction and area reviewed against the Solana security checklist.

---

## Review Methodology

Each instruction was reviewed for:
- **Signer checks**: Are all authorities verified?
- **Account validation**: PDA seeds, ownership, constraints
- **Arithmetic safety**: Checked arithmetic, overflow/underflow
- **State machine integrity**: Can states be skipped or replayed?
- **Access control**: Who can call what, and when?
- **Cryptographic soundness**: Are commitments/signatures actually verified?
- **Reentrancy / CPI safety**: Are accounts properly validated before CPI?

---

## Instruction-by-Instruction Review

### `initialize_sale.rs`

- [x] Authority is signer and payer
- [x] Sale PDA derived from `[b"sale", authority, token_mint]` — unique per authority+mint pair
- [x] Validates: raise_min <= raise_max, fdv_min <= fdv_max, supply_percentage in range, timing
- [x] Commitment window minimum enforced (currently 1 min for testing, should be 15 min)
- [x] Future timestamps required for commitment_start and commitment_end
- [x] drand_reveal_round computed from commitment_end
- **No issues found.**

### `fund_sale.rs`

- [x] Authority signer, has_one constraint
- [x] Status must be Initialized
- [x] Requires funding before commitment_start
- [x] Calculates required tokens from token_supply * supply_percentage / 10000
- [x] Transitions to Active
- **Note**: Only checks `amount >= required_tokens`, doesn't enforce exact match. Authority could overfund. Not a vulnerability — excess stays in vault and can be withdrawn later.

### `commit.rs`

- [x] Sale must be Active and not paused
- [x] Time window enforced: `commitment_start <= now <= commitment_end`
- [x] Merkle proof verified for user score (whitelist)
- [x] Amount > 0 and >= min_commitment
- [x] Encrypted bid required and <= 600 bytes
- [x] drand_round must match sale.drand_reveal_round
- [x] Cumulative amount checked against raise_max
- [x] USDC transferred via CPI with proper authority
- **CRITICAL — Salt stored on-chain** (line 131-132): `bid.salt = salt` is stored in the SealedBid account. Since accounts are readable by anyone, the salt is public. Combined with max_fdv being bounded to `[fdv_min, fdv_max]` (a finite u64 range), anyone can brute-force `SHA256(max_fdv || salt)` for every possible max_fdv value and find the match. The hash commitment provides zero privacy.
- **MEDIUM — hash_commitment and salt only set on first init** (lines 128-131): If a user calls commit again (top-up), the hash_commitment and salt are NOT updated. The encrypted bid is also not updated on subsequent commits. User cannot change their bid price after first commit.
- **HIGH — init_if_needed pattern** (lines 19-26): Uses `init_if_needed` for SealedBid PDA. While mitigated by the `is_initialized` flag check and the fact that the PDA is derived from (sale, user), Anchor docs warn about reinitialization attacks with this pattern. The mitigation appears sufficient here since the `if !bid.is_initialized` branch handles first-time init and the `else` branch validates ownership.

### `close_commitment_window.rs`

- [x] Permissionless — anyone can call
- [x] Status must be Active
- [x] Time check: `now > commitment_end`
- [x] Transitions to Revealing
- **No issues found.** Permissionless closing is correct — it's a time-gated transition.

### `batch_reveal_bids.rs`

- [x] Status must be Revealing
- [x] Permissionless cranker
- [x] Batch size limited to MAX_REVEAL_BATCH_SIZE (20)
- [x] drand_round must match sale.drand_reveal_round
- [x] Each bid PDA verified via find_program_address
- [x] Hash commitment verified: SHA256(max_fdv || salt) == hash_commitment
- [x] max_fdv must be in [fdv_min, fdv_max]
- [x] bid_revealed flag prevents double-reveal
- **CRITICAL — Drand signature not actually verified** (`drand.rs:25-45`): The `verify_drand_signature` function only checks that chain_hash matches QUICKNET_CHAIN_HASH and that the signature is not all-zeros or all-0xFF. It does NOT perform BLS12-381 signature verification against the drand public key. Anyone can submit arbitrary 48 bytes as a "valid" signature.
- **Impact**: Combined with the salt being on-chain, this means anyone can reveal any bid at any time during the Revealing phase — they just need to compute SHA256(max_fdv || salt) by trying all values in [fdv_min, fdv_max].
- **LOW — reveal_count can exceed total_users** (line 105-113): `reveal_count` is incremented by `revealed_count` without checking that it won't exceed `total_users`. Mitigated by `bid_revealed` flag preventing double-processing of the same bid.
- **False positive analysis**: The `>= total_users` check on line 107 correctly handles the boundary. Even if reveal_count slightly exceeds total_users (e.g., concurrent batches), the worst case is `all_bids_revealed` being set early, which is correct since all bids would indeed be revealed.

### `propose_settlement.rs`

- [x] Authority signer, has_one constraint
- [x] Status must be CommitmentEnded, Proposed, or Verifying (allows re-proposal)
- [x] Time check: now > commitment_end
- [x] Cooldown between proposals enforced (60s)
- [x] Initial proposal delay (30s after commitment_end)
- [x] all_bids_revealed required
- [x] clearing_fdv in [fdv_min, fdv_max]
- [x] fill_rate in (0, 10000]
- [x] Sanity checks on fill_rate vs total_committed
- [x] settlement_nonce incremented (prevents stale verifications)
- [x] Resets verification_count, running_sum, marginal_user_verified
- **Note**: The authority has significant power here — they choose the clearing_fdv. However, finalize_settlement validates that running_sum falls within [raise_min, raise_max], which constrains the authority's ability to propose unfavorable terms. If the proposal is bad, verification will fail finalization.

### `verify_and_allocate.rs`

- [x] Permissionless cranker
- [x] Status must be Proposed or Verifying
- [x] Bid must be initialized, amount > 0, not already verified for this nonce
- [x] PDA verified via seeds + bump
- [x] Bid must be revealed (bid_revealed = true, max_fdv_plaintext set)
- [x] Allocation logic: above clearing → pro-rata fill; marginal → special allocation; below → full refund
- [x] Dust protection: if allocation > 0 but tokens == 0, gives full refund
- [x] running_sum accumulated, verification_count incremented
- **Note on compilation**: The code at lines ~85-90 appears to have a syntax issue (`} else {` without matching `if`). This may be a copy issue or the code may not compile as-is. Assuming the logic is: above clearing → fill; at clearing and is marginal_user → marginal allocation; at clearing and not marginal_user → pro-rata fill; below clearing → refund.

### `finalize_settlement.rs`

- [x] Permissionless finalizer
- [x] Status must be Verifying
- [x] verification_count == total_users
- [x] marginal_user_verified if marginal_user exists
- [x] raise_min <= running_sum <= raise_max
- [x] Token vault has sufficient tokens for all allocations
- [x] Transitions to Settled
- **No issues found.** This is a solid verification step.

### `claim_allocation.rs`

- Not reviewed in detail (not in critical path for security review)
- Expected to transfer tokens + USDC refund to user based on bid.allocation and bid.refund

### `claim_refund.rs`

- [x] Status must be Cancelled or Refunding
- [x] Bid must be initialized, not claimed, amount > 0
- [x] User is signer and matches bid.user
- [x] Transfers bid.amount (full committed USDC) back to user
- [x] Account closed (rent returned to user)
- **Note**: The `amount > 0` check is in the account constraints (line 26), so it is checked. The plan mentioned "no check that amount > 0" but this is a false positive — it IS checked.

### `refund_sale.rs`

- [x] Authority signer, has_one constraint
- **HIGH — Can refund from Settled state** (line constraint): The status constraint allows `Settled` as a valid state. This means after settlement is finalized, all bids are verified, and everything looks good — the authority can still trigger refund mode. This could be used to deny users their tokens after settlement.
- **Impact**: Users who expected to receive tokens at the clearing price would instead only get their USDC back. The authority could use this to back out of an unfavorable sale after seeing the results.
- **Mitigation**: This may be intentional as an emergency escape hatch. However, it should be documented and ideally gated by a timelock or multisig.

### `cancel_sale.rs`

- Not reviewed in detail — expected to transition to Cancelled with appropriate constraints.

### `pause_sale.rs`

- Not reviewed in detail — expected to toggle is_paused with authority check.

---

## Cross-Reference: Solana Security Checklist

| Category | Status | Notes |
|----------|--------|-------|
| Missing signer checks | PASS | All authority-gated instructions require signer |
| Missing ownership checks | PASS | PDA derivation and Anchor constraints handle this |
| Missing rent exemption checks | PASS | Anchor handles via `init` |
| Arithmetic overflow/underflow | PASS | checked_add/sub/mul/div used throughout |
| Type cosplay | PASS | Anchor discriminators prevent this |
| Duplicate mutable accounts | PASS | Anchor handles via borrow checker |
| Authorization through CPI | PASS | Seed-based PDA signing for vault transfers |
| Account reinitialization | **MEDIUM** | `init_if_needed` on SealedBid — mitigated by is_initialized check |
| Closing accounts | PASS | claim_refund uses `close = user` correctly |
| PDA substitution | PASS | All PDAs verified via seeds constraints |
| Cryptographic verification | **CRITICAL** | drand BLS signature not actually verified |
| Commitment scheme soundness | **CRITICAL** | Salt stored on-chain defeats hash commitment |
| Privilege escalation | **HIGH** | Authority can refund from Settled state |
| Oracle manipulation | **CRITICAL** | Fake drand signature allows arbitrary reveals |

---

## Resolution: Salt Removal & Trustless Design

After review discussion, the following resolution was agreed:

### Changes
1. **Remove `salt` field from `SealedBid`** — no longer stored on-chain
2. **Remove `salt` parameter from `commit` instruction** — user keeps salt client-side
3. **Add `salt` parameter to `batch_reveal_bids`** — cranker provides salt obtained from decrypting timelock ciphertext
4. **Encrypted payload format**: `encrypt(max_fdv || salt, drand_round)` — both values embedded in ciphertext

### Why BLS verification is NOT needed on-chain
- SHA256 hash commitment verifies correctness of revealed max_fdv (~1000 CU)
- Submitting valid (max_fdv, salt) pair is proof of decryption (salt only exists in ciphertext)
- State machine + Clock::get() enforces timing (reveals only in Revealing state, after commitment_end)
- BLS pairing via SIMD-0129 costs ~100k-200k CU — unnecessary overhead

### Trustless property
Before drand round: nobody has salt → can't pass SHA256 check → bids sealed
After drand round: anyone decrypts → has salt → can reveal → fully permissionless

---

## False Positive Analysis

1. **`reveal_count` overflow**: Initially flagged as issue. On closer inspection, `bid_revealed` flag prevents any bid from being processed twice. The worst case is reveal_count reaching total_users from concurrent batch processing, which is handled correctly by the `>=` check.

2. **`claim_refund` missing amount check**: The constraint `sealed_bid.amount > 0` at line 26 of claim_refund.rs handles this. Not an issue.

3. **`init_if_needed` reinitialization**: The `is_initialized` flag check in the handler code provides a second layer of defense. The PDA is also unique per (sale, user). Marking as MEDIUM rather than HIGH because the mitigations are sufficient in practice, though the pattern remains risky.

4. **Authority proposing bad clearing_fdv**: Constrained by `finalize_settlement` requiring `raise_min <= running_sum <= raise_max`. A bad proposal would fail finalization. The authority can waste time but cannot steal funds through bad proposals.
