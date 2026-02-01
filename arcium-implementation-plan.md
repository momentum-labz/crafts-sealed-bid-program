# Arcium Sealed-Bid Auction — Implementation Plan (v3)

## 1. Why Arcium?

Sealed-bid auctions require bid privacy. On a transparent blockchain, bids are visible and whales can front-run. The previous drand timelock approach kept bids private during the auction but **revealed all max FDV values after the round**. Arcium MPC keeps individual bids **private forever** — only aggregate results (clearing FDV, fill rate) are revealed.

| Data | drand | Arcium |
|------|-------|--------|
| Amount committed | Public | Public |
| Max FDV during auction | Private | Private |
| Max FDV after auction | **Public** | **Private forever** |
| Clearing price | Public | Public |
| Who cleared (bool) | Public | Public |

---

## 2. Architecture

### On-Chain State

The `Sale` account stores an **MXE-encrypted array** of demand buckets:

```
Sale.encrypted_state: [[u8; 32]; STATE_LEN]   // MXE ciphertexts
Sale.state_nonce: u128                          // MXE nonce

STATE_LEN = bucket_count + 5
  - bucket_count demand buckets (configurable per sale)
  - bid_count, total_demand, 3 reserved slots
```

Each `SealedBid` stores the bidder's encrypted max FDV as a `SharedEncryptedStruct<1>`:

```
SealedBid.encryption_key: [u8; 32]     // x25519 public key
SealedBid.bid_nonce: u128              // encryption nonce
SealedBid.encrypted_max_fdv: [u8; 32]  // ciphertext
```

The encrypted data is referenced by Arcium via **byte offset + length** into the on-chain account — no serialization needed.

### Configurable Bucket Count

`bucket_count` is set per sale at initialization time. Trade-off:

| Buckets | FDV granularity ($5M-$15M range) | Encrypted state size | MPC loop iterations |
|---------|----------------------------------|---------------------|---------------------|
| 20 | $500K | 800 bytes | 20 |
| 50 | $200K | 1,760 bytes | 50 |
| 100 | $100K | 3,200 bytes | 100 |
| 200 | $50K | 6,400 bytes | 200 |

All fit within Solana's 10KB account limit. The Arcis circuits use compile-time `MAX_BUCKET_COUNT` (e.g. 200) and the settlement logic respects the actual `bucket_count` passed as a plaintext parameter.

### Four MPC Circuits (Arcis DSL)

| Circuit | Inputs | Output | When |
|---------|--------|--------|------|
| `init_auction_state` | — | `Enc<Mxe, [u64; N]>` (zeroed) | Once after sale init |
| `submit_bid` | `Enc<Shared, u64>` (bid FDV) + `Enc<Mxe, [u64; N]>` (state) + plaintext params | `Enc<Mxe, [u64; N]>` (updated) | Per bid |
| `settle_auction` | `Enc<Mxe, [u64; N]>` (state) + plaintext params | `(clearing_fdv, marginal_fill_rate_bps, demand_above, demand_marginal, total_raised, bid_count)` **revealed** | Once after commitment window |
| `check_bid_cleared` | `Enc<Shared, u64>` (bid FDV) + plaintext clearing bucket bounds | `u8` **revealed** (0=below, 1=marginal, 2=above) | Per bid (user-initiated) |

---

## 3. Conviction-Based Fill Rate

### Design Principle

Higher-conviction bidders (those who bid well above clearing) are rewarded with **100% fill**. Only bidders in the marginal bucket (the clearing bucket) share the remaining capacity pro-rata.

```
┌─────────────────────────────────────────────────────────┐
│  FDV Range          Demand    Fill Rate                  │
│  ─────────────────  ───────   ──────────                 │
│  $14M-$15M (top)    $200K     100%  ← conviction reward │
│  $13M-$14M          $500K     100%  ← conviction reward │
│  $12M-$13M          $800K     100%  ← conviction reward │
│  $11M-$12M          $1.2M     100%  ← conviction reward │
│  ───── clearing bucket ─────                             │
│  $10M-$11M          $3M       42%   ← marginal, pro-rata│
│  ───── below clearing ──────                             │
│  $9M-$10M           $1.5M     0%    ← full refund       │
│  $8M-$9M            $900K     0%    ← full refund       │
│  ...                                                     │
└─────────────────────────────────────────────────────────┘
```

### Settlement Outputs

The `settle_auction` circuit reveals:

| Output | Description |
|--------|-------------|
| `clearing_fdv` | Midpoint of clearing bucket |
| `marginal_fill_rate_bps` | Pro-rata fill for marginal bucket only |
| `demand_above_clearing` | Total USDC committed above clearing bucket |
| `demand_in_marginal` | Total USDC committed in clearing bucket |
| `total_raised` | Actual USDC raised |
| `bid_count` | Total number of bids |

### Per-Bid Check Result

The `check_bid_cleared` circuit returns a `u8`:

| Value | Meaning | Fill Rate | Privacy Leak |
|-------|---------|-----------|-------------|
| 0 | Below clearing bucket | 0% — full refund | "Bid was below clearing" |
| 1 | In clearing bucket (marginal) | `marginal_fill_rate_bps` | "Bid was in [$X-$Y] range" |
| 2 | Above clearing bucket | 100% — full fill | "Bid was above clearing" |

**Privacy trade-off:** Value 1 reveals the bidder was in a specific FDV range (one bucket width). This is acceptable — it's a narrow window and the least interesting position. The real sensitive info (whales bidding 5x above clearing) stays hidden behind value 2.

### Allocation Calculation (in callback)

```rust
match bid_result {
    0 => {  // Below clearing
        bid.allocation = 0;
        bid.refund = bid.amount;
        bid.tokens = 0;
    }
    1 => {  // Marginal — pro-rata within clearing bucket
        bid.allocation = (bid.amount * marginal_fill_rate) / 10000;
        bid.tokens = calculate_tokens(bid.allocation, token_supply, clearing_fdv);
        bid.refund = bid.amount - bid.allocation;
    }
    2 => {  // Above clearing — 100% fill, conviction rewarded
        bid.allocation = bid.amount;
        bid.tokens = calculate_tokens(bid.allocation, token_supply, clearing_fdv);
        bid.refund = 0;
    }
}
```

### Marginal Fill Rate Calculation (in settle_auction circuit)

```
tokens_for_sale = (token_supply * supply_pct_bps) / 10000
raise_target = (tokens_for_sale * clearing_fdv) / token_supply

demand_above = sum of all buckets strictly above clearing bucket
remaining_capacity = raise_target - demand_above

marginal_fill_rate_bps = min(10000, (remaining_capacity * 10000) / demand_in_clearing_bucket)
```

If `demand_above` already exceeds `raise_target`, the marginal fill rate is 0 (clearing bucket gets nothing). If `demand_above` + `demand_in_clearing_bucket` is under `raise_target`, marginal fill rate is 10000 (100%).

---

## 4. Sale Lifecycle

```
initialize_sale(bucket_count=100, fdv_min, fdv_max, ...)
    |
fund_sale()
    |
queue_init_state()  --callback-->  Sale.encrypted_state = zeroed MXE blob
    |
    |  [commitment window open]
    |
    |-- commit() x N          Store USDC + encrypted_max_fdv on SealedBid
    |-- queue_submit_bid() x N --callback-->  encrypted_state updated per bid
    |
close_commitment_window()
    |
trigger_settlement()  --callback-->  Sale.clearing_fdv, marginal_fill_rate, totals (plaintext)
    |
enable_claims()       Authority go/no-go switch
    |
    |-- [each user self-service]
    |-- queue_check_bid()  --callback-->  SealedBid: bid_status (0/1/2), allocation, refund, tokens
    |-- claim_allocation() or claim_refund()
```

### Status Flow

```
Initialized --> Active --> CommitmentEnded --> Ready --> Settling --> Settled --> CheckingBids --> Finalized
                  |                                                                                  |
                  +--> Cancelled --> Refunding                                                        +--> claims
```

### Self-Service Bid Checking

After settlement, **each user calls `queue_check_bid` on their own bid**, paying for the MPC computation. This avoids the need for a centralized operator to batch-check 10k bids. The MPC circuit is trivial (decrypt one u64, two comparisons), so cost per user is minimal.

The `enable_claims` step remains as an authority-controlled gate — the authority must explicitly enable claims before any user can withdraw. This allows for a review period after settlement.

---

## 5. Per-User Cost (10k bids scenario)

For a single user checking and claiming:

| Step | Who pays | Cost |
|------|----------|------|
| `queue_check_bid` | User | 1 MPC round + 2 Solana txs |
| `claim_allocation` | User | 1 Solana tx |

The `check_bid_cleared` circuit is a single u64 decrypt + two comparisons — the cheapest possible MPC operation. No O(N) work; each user's check is independent.

System-level costs (paid by sale authority or operator):

| Step | Count | Cost |
|------|-------|------|
| `queue_init_state` | 1 | 1 MPC round |
| `queue_submit_bid` | 10,000 | 10k MPC rounds (can be parallelized) |
| `trigger_settlement` | 1 | 1 MPC round (O(bucket_count) internally) |
| `enable_claims` | 1 | 1 Solana tx |

The expensive part is 10k `submit_bid` MPC calls during the commitment window. These can be queued in parallel since each updates the same encrypted state sequentially via Arcium's ordering.

---

## 6. Client-Side Key Management

### Bid Encryption

Users encrypt their `max_fdv` using x25519 key exchange with the MXE public key:

```ts
const sig = await wallet.signMessage("arcium-fundraising-key");
const x25519Private = sha256(sig);  // deterministic per wallet
const x25519Public = x25519.getPublicKey(x25519Private);
const sharedSecret = x25519.getSharedSecret(x25519Private, mxePublicKey);
const cipher = new RescueCipher(sharedSecret);
const encrypted = cipher.encrypt(maxFdvBytes, nonce);
```

### Displaying Bid FDV After Settlement (Option A — deterministic key derivation)

The user's x25519 private key is deterministically derived from their wallet signature. When they return to check results:

1. Sign the same fixed message → same x25519 private key
2. Read `encrypted_max_fdv` + `encryption_key` + `bid_nonce` from their `SealedBid` account
3. Re-derive shared secret, decrypt locally
4. Display: "Your max FDV was $X. The clearing FDV was $Y."

**UX cost:** One "Sign message" prompt per browser session (same as Tensor, Magic Eden session keys). No extra signature if still in the session where they committed.

The contract never reveals the plaintext max FDV — the user recovers it client-side from their own encrypted data on-chain.

---

## 7. Account Structures

### Sale Account

```rust
pub struct Sale {
    // Identity
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub usdc_mint: Pubkey,
    pub usdc_vault: Pubkey,
    pub token_vault: Pubkey,

    // Parameters
    pub token_supply: u64,
    pub supply_percentage: u16,      // bps
    pub raise_min: u64,
    pub raise_max: u64,
    pub fdv_min: u64,
    pub fdv_max: u64,
    pub min_commitment: u64,
    pub bucket_count: u16,           // configurable per sale (1-200)

    // Timing
    pub commitment_start: i64,
    pub commitment_end: i64,

    // Score verification
    pub score_merkle_root: [u8; 32],

    // Settlement results (set by Arcium callback)
    pub clearing_fdv: Option<u64>,
    pub clearing_bucket_low: Option<u64>,   // lower FDV bound of clearing bucket
    pub clearing_bucket_high: Option<u64>,  // upper FDV bound of clearing bucket
    pub marginal_fill_rate: Option<u64>,    // bps, applies only to marginal bucket
    pub total_raised: Option<u64>,

    // Counters
    pub total_users: u32,
    pub total_committed: u64,
    pub settlement_nonce: u64,
    pub bids_checked: u32,

    // Status
    pub status: SaleStatus,
    pub is_paused: bool,
    pub claims_enabled: bool,
    pub arcium_settled: bool,

    // Arcium MXE encrypted state (contiguous for account() reference)
    pub state_nonce: u128,
    pub encrypted_state: [[u8; 32]; MAX_BUCKET_COUNT + 5],

    // Metadata
    pub name: String,
    pub created_at: i64,
    pub bump: u8,
}
```

### SealedBid Account

```rust
pub struct SealedBid {
    pub sale: Pubkey,
    pub user: Pubkey,
    pub amount: u64,                   // plaintext USDC committed
    pub commit_time: i64,

    // Arcium encrypted bid (SharedEncryptedStruct<1>)
    pub encryption_key: [u8; 32],      // x25519 public key
    pub bid_nonce: u128,               // encryption nonce
    pub encrypted_max_fdv: [u8; 32],   // ciphertext

    // Post-settlement (set by check_bid_cleared callback)
    pub bid_status: u8,                // 0=below, 1=marginal, 2=above
    pub bid_checked: bool,
    pub allocation: u64,
    pub refund: u64,
    pub tokens: u64,

    // Lifecycle
    pub is_initialized: bool,
    pub claimed: bool,
    pub bump: u8,
}
```

---

## 8. Instruction Summary

### Setup (once per program deployment)

| Instruction | Purpose |
|-------------|---------|
| `init_init_auction_state_comp_def` | Register circuit with Arcium |
| `init_submit_bid_comp_def` | Register circuit with Arcium |
| `init_settle_auction_comp_def` | Register circuit with Arcium |
| `init_check_bid_cleared_comp_def` | Register circuit with Arcium |

### Sale Lifecycle

| Instruction | Who | Purpose |
|-------------|-----|---------|
| `initialize_sale` | Authority | Create sale with configurable `bucket_count` |
| `fund_sale` | Authority | Fund token vault |
| `queue_init_state` | Authority | MPC: create zeroed encrypted state |
| `commit` | User | Deposit USDC + encrypted max FDV |
| `queue_submit_bid` | Authority/Operator | MPC: add bid to encrypted buckets |
| `close_commitment_window` | Authority | End commitment period |
| `trigger_settlement` | Authority | MPC: compute clearing + marginal fill rate |
| `enable_claims` | Authority | Allow users to claim |
| `queue_check_bid` | User (self-service) | MPC: classify bid (above/marginal/below) |
| `claim_allocation` | User | Withdraw tokens + USDC refund |
| `claim_refund` | User | Full refund (cancelled/refunding sales) |

### Callbacks (called by Arcium ARX nodes)

| Callback | Writes to |
|----------|-----------|
| `init_auction_state_callback` | `Sale.state_nonce`, `Sale.encrypted_state` |
| `submit_bid_callback` | `Sale.state_nonce`, `Sale.encrypted_state` |
| `settle_auction_callback` | `Sale.clearing_fdv`, `.clearing_bucket_low/high`, `.marginal_fill_rate`, `.total_raised` |
| `check_bid_cleared_callback` | `SealedBid.bid_status`, `.allocation`, `.refund`, `.tokens` |

---

## 9. What's Implemented vs. TODO

### Done

- [x] 4 Arcis circuits (`encrypted-ixs/src/lib.rs`) — compile via `arcium build`
- [x] All on-chain instructions (queue + callback pairs)
- [x] Comp def initializers
- [x] State structures with encrypted fields
- [x] Box<Account> on Sale to fix SBF stack overflow
- [x] `arcium-client` default-features disabled for SBF compatibility
- [x] Registry patches for const-oid, time-core, getrandom on SBF
- [x] Test file (`tests/arcium_fundraising.ts`) with 10-bidder flow
- [x] Program builds successfully (`anchor build`)

### TODO

- [ ] **Conviction-based fill rate** — replace uniform fill_rate with marginal-bucket-only pro-rata
  - Update `settle_auction` circuit to output `marginal_fill_rate_bps`, `clearing_bucket_low`, `clearing_bucket_high`
  - Update `check_bid_cleared` circuit to return `u8` (0/1/2) instead of `bool`
  - Update `check_bid_cleared_callback` to apply 100%/marginal/0% based on `bid_status`
  - Update `Sale` account: replace `fill_rate` with `marginal_fill_rate`, add `clearing_bucket_low/high`
  - Update `SealedBid` account: replace `cleared: bool` with `bid_status: u8`
- [ ] **Make `bucket_count` configurable per sale** — currently hardcoded to 20
  - Add `bucket_count: u16` param to `initialize_sale`
  - Arcis circuits use `MAX_BUCKET_COUNT` constant, pass actual count as plaintext
  - `submit_bid` circuit: loop up to MAX, guard with `if j < bucket_count` inside loop
  - `settle_auction` circuit: same approach
  - Sale account sized for MAX_BUCKET_COUNT, actual count stored in field
- [ ] **Fix `arcium test` localnet startup** — validator crashes silently when launched via arcium CLI (works fine via `anchor localnet` directly)
- [ ] **Client-side key derivation** — implement Option A (deterministic x25519 from wallet signature) in test/frontend
- [ ] **Run full 10-bidder test end-to-end**
- [ ] **Test with higher bucket counts** (50, 100, 200)
- [ ] **Edge cases**: undersubscribed sale, all bids below min FDV, single bidder, exact clearing boundary, demand_above > raise_target (marginal gets 0%)
- [ ] **Remove unused imports** (`CallbackError`, `ID_CONST`) flagged by compiler warnings
- [ ] **Security review** — verify callback validation, overflow checks, account constraints

---

## 10. Open Design Questions

1. **Who pays for `queue_submit_bid`?** Currently the authority/operator batches these after each `commit`. Alternative: user queues their own submit_bid as part of the commit flow (user pays MPC cost).

2. **Parallel submit_bid ordering** — Arcium processes MPC sequentially for state updates. With 10k bids, throughput depends on MPC round latency. May need to measure.

3. **Bucket count upper bound** — What's the practical MPC limit? 200 loop iterations in Arcis should be fine, but needs testing. The on-chain state for 200 buckets is ~6.5KB (fits in one account).

4. **Fallback if Arcium is down** — Currently no fallback. Could add a timeout-based cancel path if settlement callback doesn't arrive within N hours.

5. **Edge case: demand_above > raise_target** — If bids above the clearing bucket already exceed the raise target, the marginal bucket gets 0% fill. Should this trigger re-settlement with the clearing bucket shifted up? Current design: no, marginal gets 0% and the sale is slightly oversubscribed from above-clearing bids. This rewards conviction maximally.
