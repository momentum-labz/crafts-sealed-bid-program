# V0.2 Drand Sealed-Bid Auction Flow

## All On-Chain Instructions

### Core Drand Flow (Tested ✓)
1. `initialize_sale` - Creates sale with drand reveal round
2. `fund_sale` - Funds sale with tokens
3. `commit` - Users submit encrypted bids
4. `close_commitment_window` - Transitions to Revealing status
5. `batch_reveal_bids` - Submits decrypted bids with drand signature
6. `propose_settlement` - Proposes clearing price
7. `batch_verify_and_allocate` - Verifies users in batches
8. `finalize_settlement` - Finalizes the sale
9. `enable_claims` - Enables users to claim
10. `claim_allocation` - Users claim tokens/refunds

### Additional Instructions (Not tested yet)
11. `verify_and_allocate` - Single user verification (we use batch version)
12. `cancel_sale` - Authority cancels before settlement
13. `claim_refund` - Claim refund after cancellation
14. `withdraw_unsold` - Authority withdraws unsold tokens
15. `withdraw_raised` - Authority withdraws raised USDC
16. `pause_sale` - Emergency pause
17. `unpause_sale` - Resume after pause
18. `refund_sale` - Trigger full refund mode

---

## Complete Drand Encryption Flow

### Phase 1: Before Commitment (OFF-CHAIN)

**User prepares their bid:**
1. User decides their max FDV (e.g., $2,000,000)
2. User knows the commitment window ends at timestamp `T`
3. Calculate drand round: `round = (T - GENESIS) / PERIOD`
   - Quicknet: `GENESIS = 1692803367`, `PERIOD = 3 seconds`
   - Example: If `T = 1769468666`, then `round = (1769468666 - 1692803367) / 3 = 25,555,099`

**Encryption happens:**
```javascript
import { timelockEncrypt } from 'tlock-js';

// User's secret bid
const maxFdv = 2_000_000 * 1e6; // $2M in 6 decimals

// Convert to buffer
const message = Buffer.alloc(8);
message.writeBigUInt64LE(BigInt(maxFdv));

// Encrypt with future drand round
const ciphertext = await timelockEncrypt(
  drandRevealRound,           // Future round number
  message,                     // Secret max_fdv
  QUICKNET_CHAIN_HASH         // Drand chain identifier
);
```

**What just happened?**
- `timelockEncrypt` uses drand's public key and round number
- Creates a ciphertext that can ONLY be decrypted after that specific drand round is published
- The encrypted blob is ~256 bytes
- Nobody can decrypt it until the drand network publishes the randomness for that round

### Phase 2: Commitment Window (ON-CHAIN)

**User submits encrypted bid:**
```typescript
await program.methods
  .commit(
    new BN(100_000 * 1e6),      // amount (USDC)
    Array.from(ciphertext),      // max_fdv_encrypted (the encrypted blob!)
    new BN(drandRevealRound),    // which round was used for encryption
    score,                       // user's score
    proofArrays                  // Merkle proof
  )
  .accounts({...})
  .signers([user])
  .rpc();
```

**On-chain storage:**
```rust
pub struct SealedBid {
    pub amount: u64,                        // 100,000 USDC (visible)
    pub max_fdv_encrypted: Option<Vec<u8>>, // Encrypted blob (hidden!)
    pub max_fdv_plaintext: Option<u64>,     // None (not revealed yet)
    pub drand_round: u64,                   // Round number
    pub bid_revealed: bool,                 // false
    ...
}
```

**Key point:** The max_fdv is ENCRYPTED on-chain. Nobody knows the user's true max FDV yet!

### Phase 3: Close Commitment Window (ON-CHAIN)

After commitment window ends:
```typescript
await program.methods
  .closeCommitmentWindow()
  .accounts({...})
  .rpc();
```

This transitions the sale from `Active` → `Revealing` status.

### Phase 4: Reveal Bids (OFF-CHAIN + ON-CHAIN)

**Step 1: Fetch drand randomness (OFF-CHAIN)**
```javascript
import { FastestNodeClient } from 'drand-client';

// Initialize client
const client = new FastestNodeClient([...]);

// Fetch the randomness for the reveal round
const randomness = await client.get(drandRevealRound);
// randomness.signature is the BLS signature (48 bytes)
```

**What just happened?**
- The drand network has now published the randomness for round `drandRevealRound`
- This randomness is the "key" that unlocks all the encrypted bids
- Anyone in the world can fetch this - it's public!

**Step 2: Decrypt bids (OFF-CHAIN)**
```javascript
import { timelockDecrypt } from 'tlock-js';

// For each encrypted bid:
const bid = await program.account.sealedBid.fetch(bidPda);
const encryptedData = Buffer.from(bid.maxFdvEncrypted);

// Decrypt using the drand randomness
const decrypted = await timelockDecrypt(
  encryptedData,
  randomness
);

// Extract the max_fdv
const maxFdv = Number(Buffer.from(decrypted).readBigUInt64LE(0));
// maxFdv = 2000000000000 ($2M)
```

**Step 3: Submit revealed bids (ON-CHAIN)**
```typescript
const revealedBids = [
  { user: user1.publicKey, maxFdv: new BN(2_000_000 * 1e6) },
  { user: user2.publicKey, maxFdv: new BN(1_500_000 * 1e6) },
  { user: user3.publicKey, maxFdv: new BN(1_000_000 * 1e6) },
];

await program.methods
  .batchRevealBids(
    new BN(drandRevealRound),        // The round number
    Array.from(randomness.signature), // The drand BLS signature (proof!)
    revealedBids                      // Decrypted max_fdv values
  )
  .accounts({...})
  .remainingAccounts([bid1Pda, bid2Pda, bid3Pda])
  .rpc();
```

**On-chain verification:**
```rust
// The program verifies the drand signature
verify_drand_signature(&sale.drand_chain_hash, drand_round, &drand_signature)?;

// Then writes the plaintext values
bid.max_fdv_plaintext = Some(revealed_bid.max_fdv);
bid.bid_revealed = true;
```

**Key point:** The on-chain program verifies the drand signature to ensure we're using real randomness from the correct round!

### Phase 5: Settlement (ON-CHAIN)

Now that bids are revealed, settlement proceeds normally:
1. `propose_settlement` - Uses revealed `max_fdv_plaintext` values
2. `batch_verify_and_allocate` - Allocates based on revealed FDVs
3. `finalize_settlement`
4. `enable_claims`
5. `claim_allocation`

---

## Why This Flow is Secure

### Before Reveal:
- ✓ Bids are encrypted with timelock encryption
- ✓ Nobody can decrypt them until the specific drand round is published
- ✓ Not even the user who encrypted it can decrypt early
- ✓ Front-running is impossible - bids are hidden

### After Reveal:
- ✓ Drand signature proves the randomness is authentic
- ✓ Anyone can verify the decryption was done correctly
- ✓ The reveal is permissionless - anyone can submit it
- ✓ All bids are revealed simultaneously

### Drand's Role:
- Drand is a decentralized randomness beacon
- Runs by a consortium of institutions (Cloudflare, EPFL, etc.)
- Publishes randomness every 3 seconds on Quicknet
- Uses BLS signatures - verifiable on-chain
- Makes timelock encryption possible!

---

## Test Coverage

### Currently Tested (Happy Path):
1. ✓ Initialize sale with drand configuration
2. ✓ Fund sale
3. ✓ Users commit with encrypted bids
4. ✓ Close commitment window
5. ✓ Batch reveal bids using real drand
6. ✓ Propose settlement
7. ✓ Batch verify and allocate
8. ✓ Finalize settlement
9. ✓ Enable claims
10. ✓ Claim allocation

### Not Yet Tested (Edge Cases):
- ❌ Cancel sale before reveal
- ❌ Pause/unpause during commitment
- ❌ Refund mode
- ❌ Withdraw unsold tokens
- ❌ Withdraw raised funds
- ❌ Single verify_and_allocate (we only test batch)
- ❌ Claim refund after cancellation

### Recommendations:
Consider adding tests for:
1. What happens if commitment is cancelled before reveal?
2. Can we pause during commitment window?
3. What if someone tries to reveal with wrong drand signature?
4. What if someone tries to reveal before commitment ends?
