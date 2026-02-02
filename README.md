# Crafts — Sealed Bid Uniform Price Auction on Solana

On-chain sealed-bid token auctions with uniform clearing price settlement, designed for ICO scenarios wiht 10k+ participants.

## What It Does

Projects fund a token sale with configurable FDV bounds and raise targets. Bidders commit USDC with encrypted max FDV values. After the commitment window closes, bids are revealed (or computed privately), a clearing price is proposed, and all bids are settled at that uniform price. Bidders above the clearing FDV get tokens; those below get refunded.

## Implementations

### v2 — drand (`fundraising_program/`)

Bid privacy via timelock encryption using drand Quicknet beacons.

- Bidder commits USDC + SHA256-hashed bid (max FDV + salt)
- Commitment window closes, drand round becomes available
- Crankers reveal bids using the drand signature
- Authority proposes clearing FDV, crankers verify allocations
- Settlement finalizes; bidders claim tokens or refunds

### v3 — Arcium MPC (`arcium_fundraising_program/`)

Bid privacy via Arcium's Multi-Party Computation — bids stay encrypted forever.

- Bids are submitted as MXE-encrypted demand buckets on-chain
- No reveal phase — Arcium nodes compute the clearing price over encrypted data
- Settlement result is returned to the program without exposing individual bids
- Configurable bucket granularity (20, 50, or 100 buckets)

## Experimental Integrations

### MagicBlock (`magicblock_fundraising_program/`)

Uses MagicBlock's ephemeral rollups to keep bids off the base layer entirely. Bid accounts are delegated to an ephemeral validator where bidders submit and update bids privately. After the commitment window, the program closes and computes on the ephemeral state, then undelegates results back to mainnet for settlement.

### RadrLabs / Surfpool

Each program directory includes a `Surfpool.toml` for local development with Surfpool, RadrLabs' Solana development simulator. Provides fast local iteration without spinning up a full validator.

### Helius

RPC provider configuration included in Surfpool configs for mainnet fork testing.

## Clearing Price

The authority proposes a clearing FDV within the sale's configured bounds. Every bid at or above this FDV is filled at the clearing price (uniform price — no one overpays). If total demand at the clearing FDV exceeds the raise cap, a fill rate is applied so marginal bidders receive a pro-rata allocation and the remainder is refunded.

## Repo Structure

```
hackathon_submission/
├── fundraising_program/           # v2 — drand timelock
│   └── programs/fundraising_program/
├── arcium_fundraising_program/    # v3 — Arcium MPC
│   ├── programs/fundraising_program/
│   └── encrypted-ixs/
├── magicblock_fundraising_program/ # Experimental — MagicBlock ER
│   └── programs/fundraising_mb/
├── PROCESS.md                     # Detailed auction lifecycle docs
└── README.md
```

## Sponsors

| Sponsor | Integration | Where |
|---------|-------------|-------|
| **Arcium** | MPC-based encrypted bid computation | `arcium_fundraising_program/` |
| **MagicBlock** | Ephemeral rollups for off-chain bid privacy | `magicblock_fundraising_program/` |
| **RadrLabs** | Surfpool local dev simulator | `Surfpool.toml` in each program |
| **Helius** | RPC provider for mainnet fork testing | `Surfpool.toml` configs |

## Build

```bash
# Each program directory is an independent Anchor workspace

cd fundraising_program
anchor build

cd ../arcium_fundraising_program
anchor build

cd ../magicblock_fundraising_program
anchor build
```
