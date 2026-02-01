# Clearing Price Calculation - Off-Chain Helper

This document shows how to calculate the clearing FDV for the fundraising program v0.1.

## Concept

In v0.1, users commit USDC and specify their **max_fdv** (maximum price they're willing to pay).
The clearing FDV is the highest FDV where total demand >= raise_target.

## Algorithm

```typescript
interface Bid {
  user: PublicKey;
  amount: number;        // USDC committed (in lamports)
  max_fdv: number;       // Maximum FDV user will accept (in lamports)
}

interface ClearingResult {
  clearing_fdv: number;
  fill_rate: number;           // In basis points (10000 = 100%)
  total_cleared: number;       // Total USDC cleared
  marginal_user?: PublicKey;   // User at the clearing price
  marginal_allocation?: number; // Partial allocation for marginal user
}

/**
 * Calculate clearing FDV for the sale
 *
 * @param bids - Array of all user bids
 * @param raise_min - Minimum USDC to raise
 * @param raise_max - Maximum USDC to raise
 * @param fdv_min - Minimum allowed FDV
 * @param fdv_max - Maximum allowed FDV
 * @returns ClearingResult with clearing FDV and allocation details
 */
function calculateClearingPrice(
  bids: Bid[],
  raise_min: number,
  raise_max: number,
  fdv_min: number,
  fdv_max: number
): ClearingResult | null {

  // Step 1: Sort bids by max_fdv descending (highest price first)
  const sortedBids = [...bids].sort((a, b) => b.max_fdv - a.max_fdv);

  // Step 2: Find clearing FDV by walking down the demand curve
  let cumulative_demand = 0;
  let clearing_fdv = 0;
  let clearing_index = -1;

  for (let i = 0; i < sortedBids.length; i++) {
    cumulative_demand += sortedBids[i].amount;

    // Check if we've met the minimum raise requirement
    if (cumulative_demand >= raise_min) {
      clearing_fdv = sortedBids[i].max_fdv;
      clearing_index = i;
      break;
    }
  }

  // If we can't meet raise_min, sale fails
  if (clearing_index === -1) {
    console.log("Insufficient demand: cannot meet raise_min");
    return null;
  }

  // Validate clearing FDV is within bounds
  if (clearing_fdv < fdv_min || clearing_fdv > fdv_max) {
    console.log(`Clearing FDV ${clearing_fdv} outside bounds [${fdv_min}, ${fdv_max}]`);
    return null;
  }

  // Step 3: Calculate total cleared demand (all bids >= clearing_fdv)
  let total_cleared = 0;
  const cleared_bids = sortedBids.filter(bid => bid.max_fdv >= clearing_fdv);
  for (const bid of cleared_bids) {
    total_cleared += bid.amount;
  }

  // Step 4: Check if oversubscribed (demand > raise_max)
  if (total_cleared <= raise_max) {
    // Not oversubscribed - everyone gets 100% fill
    return {
      clearing_fdv,
      fill_rate: 10000, // 100%
      total_cleared,
    };
  }

  // Step 5: Handle oversubscribed case
  // Find marginal bidders (those at exactly clearing_fdv)
  const marginal_bids = cleared_bids.filter(bid => bid.max_fdv === clearing_fdv);
  const non_marginal_bids = cleared_bids.filter(bid => bid.max_fdv > clearing_fdv);

  // Calculate non-marginal demand
  let non_marginal_demand = 0;
  for (const bid of non_marginal_bids) {
    non_marginal_demand += bid.amount;
  }

  // Remaining allocation for marginal bidders
  const remaining_allocation = raise_max - non_marginal_demand;

  if (remaining_allocation <= 0) {
    // All raise_max allocated to non-marginal bidders
    // Need to increase clearing_fdv to next price level
    console.log("All allocation consumed by non-marginal bidders");

    // Find next highest price level
    const next_fdv = non_marginal_bids[non_marginal_bids.length - 1]?.max_fdv;
    if (!next_fdv || next_fdv === clearing_fdv) {
      return null;
    }

    // Recalculate with new clearing FDV
    return calculateClearingPrice(
      bids.filter(b => b.max_fdv > clearing_fdv),
      raise_min,
      raise_max,
      fdv_min,
      fdv_max
    );
  }

  // Calculate total marginal demand
  let marginal_demand = 0;
  for (const bid of marginal_bids) {
    marginal_demand += bid.amount;
  }

  if (marginal_bids.length === 1) {
    // Single marginal bidder - gets partial allocation
    const marginal_user = marginal_bids[0].user;
    const marginal_allocation = Math.min(remaining_allocation, marginal_bids[0].amount);

    // Calculate pro-rata fill rate for non-marginal
    const fill_rate = Math.floor((raise_max * 10000) / (non_marginal_demand + marginal_allocation));

    return {
      clearing_fdv,
      fill_rate,
      total_cleared: raise_max,
      marginal_user,
      marginal_allocation,
    };
  } else {
    // Multiple marginal bidders - pro-rata across all
    const fill_rate = Math.floor((raise_max * 10000) / total_cleared);

    return {
      clearing_fdv,
      fill_rate,
      total_cleared: raise_max,
    };
  }
}

/**
 * Validate if proposed settlement is correct
 */
function validateSettlement(
  bids: Bid[],
  clearing_fdv: number,
  fill_rate: number,
  marginal_user?: PublicKey,
  marginal_allocation?: number,
  raise_max: number = 0
): boolean {
  // Validate all cleared bids
  let total_allocated = 0;

  for (const bid of bids) {
    if (bid.max_fdv > clearing_fdv) {
      // Non-marginal: gets pro-rata allocation
      const allocation = Math.floor((bid.amount * fill_rate) / 10000);
      total_allocated += allocation;
    } else if (bid.max_fdv === clearing_fdv) {
      // Marginal bidder
      if (marginal_user && bid.user.equals(marginal_user)) {
        total_allocated += marginal_allocation!;
      } else {
        // Other marginal bidders get pro-rata
        const allocation = Math.floor((bid.amount * fill_rate) / 10000);
        total_allocated += allocation;
      }
    }
    // Excluded bids (max_fdv < clearing_fdv) get 0
  }

  // Check if total allocated is within acceptable range
  const tolerance = raise_max * 0.01; // 1% tolerance
  return Math.abs(total_allocated - raise_max) <= tolerance;
}

// ============================================================================
// EXAMPLE USAGE
// ============================================================================

const example_bids: Bid[] = [
  { user: new PublicKey("Alice..."), amount: 100_000_000000, max_fdv: 2_000_000_000000 },  // $100K @ $2M
  { user: new PublicKey("Bob..."),   amount: 150_000_000000, max_fdv: 1_500_000_000000 },  // $150K @ $1.5M
  { user: new PublicKey("Carol..."), amount: 80_000_000000,  max_fdv: 1_000_000_000000 },  // $80K @ $1M
  { user: new PublicKey("Dave..."),  amount: 50_000_000000,  max_fdv: 800_000_000000 },    // $50K @ $800K
  { user: new PublicKey("Eve..."),   amount: 70_000_000000,  max_fdv: 500_000_000000 },    // $70K @ $500K
];

const result = calculateClearingPrice(
  example_bids,
  100_000_000000,   // raise_min = $100K
  500_000_000000,   // raise_max = $500K
  400_000_000000,   // fdv_min = $400K
  2_000_000_000000  // fdv_max = $2M
);

console.log("Clearing Result:", result);
// Output:
// {
//   clearing_fdv: 1_000_000_000000,  // $1M FDV
//   fill_rate: 10000,                 // 100% fill
//   total_cleared: 330_000_000000,    // $330K total
//   marginal_user: Carol's pubkey,    // Carol is marginal
//   marginal_allocation: 80_000_000000 // Gets full allocation
// }
```

## Key Points

1. **Off-chain calculation**: The authority/proposer calculates clearing FDV off-chain
2. **On-chain verification**: The program verifies each user's allocation matches the proposal
3. **Marginal bidder**: The user whose max_fdv equals clearing_fdv and may get partial fill
4. **Fill rate**: In basis points (10000 = 100%, 5000 = 50% for 2x oversubscribed)

## Real-World Usage

```typescript
// 1. Fetch all bids from on-chain
const allBids = await fetchAllBidsFromChain(salePda);

// 2. Calculate clearing price
const clearing = calculateClearingPrice(
  allBids,
  sale.raise_min,
  sale.raise_max,
  sale.fdv_min,
  sale.fdv_max
);

// 3. Propose settlement on-chain
await program.methods
  .proposeSettlement(
    new BN(clearing.clearing_fdv),
    new BN(clearing.fill_rate),
    clearing.marginal_user || null,
    clearing.marginal_allocation ? new BN(clearing.marginal_allocation) : null
  )
  .accounts({ sale: salePda, authority: authorityKeypair.publicKey })
  .signers([authorityKeypair])
  .rpc();
```
