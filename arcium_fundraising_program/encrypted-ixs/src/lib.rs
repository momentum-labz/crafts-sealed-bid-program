use arcis::encrypted;

#[encrypted]
mod circuits {
    use arcis::*;

    const MAX_BUCKET_COUNT: usize = 100;
    const STATE_LEN: usize = MAX_BUCKET_COUNT + 5;

    #[instruction]
    fn init_auction_state() -> Enc<Mxe, [u64; STATE_LEN]> {
        let state = [0u64; STATE_LEN];
        Mxe::get().from_arcis(state)
    }

    #[instruction]
    fn submit_bid(
        encrypted_max_fdv: Enc<Shared, u64>,
        state: Enc<Mxe, [u64; STATE_LEN]>,
        amount: u64,
        fdv_min: u64,
        fdv_max: u64,
        bucket_count: u64,
    ) -> Enc<Mxe, [u64; STATE_LEN]> {
        let max_fdv = encrypted_max_fdv.to_arcis();
        let mut buckets = state.to_arcis();

        let range = fdv_max - fdv_min;
        let bucket_size = range / bucket_count;

        let clamped_fdv = if max_fdv < fdv_min {
            fdv_min
        } else if max_fdv > fdv_max {
            fdv_max
        } else {
            max_fdv
        };

        let offset = clamped_fdv - fdv_min;
        let mut bucket_idx = offset / bucket_size;
        if bucket_idx >= bucket_count {
            bucket_idx = bucket_count - 1;
        }

        for j in 0..MAX_BUCKET_COUNT {
            if (j as u64) < bucket_count {
                let is_target = (j as u64 == bucket_idx) as u64;
                buckets[j] = buckets[j] + (amount * is_target);
            }
        }

        buckets[MAX_BUCKET_COUNT] = buckets[MAX_BUCKET_COUNT] + 1;
        buckets[MAX_BUCKET_COUNT + 1] = buckets[MAX_BUCKET_COUNT + 1] + amount;

        state.owner.from_arcis(buckets)
    }

    #[instruction]
    fn settle_auction(
        state: Enc<Mxe, [u64; STATE_LEN]>,
        fdv_min: u64,
        fdv_max: u64,
        token_supply: u64,
        supply_pct_bps: u64,
        bucket_count: u64,
    ) -> (u64, u64, u64, u64, u64, u64) {
        let buckets = state.to_arcis();

        let bid_count = buckets[MAX_BUCKET_COUNT];
        let range = fdv_max - fdv_min;
        let bucket_size = range / bucket_count;
        let tokens_for_sale = (token_supply * supply_pct_bps) / 10000;

        let mut accumulated_demand: u64 = 0;
        let mut clearing_bucket: u64 = bucket_count - 1;
        let mut found: u64 = 0;

        for i in 0..MAX_BUCKET_COUNT {
            if (i as u64) < bucket_count {
                let b = bucket_count - 1 - (i as u64);
                // Index into array: we need to convert b back to usize for indexing
                // Since b < bucket_count <= MAX_BUCKET_COUNT, we iterate and match
                let mut bucket_demand: u64 = 0;
                for k in 0..MAX_BUCKET_COUNT {
                    if k as u64 == b {
                        bucket_demand = buckets[k];
                    }
                }
                accumulated_demand = accumulated_demand + bucket_demand;

                let bucket_fdv = fdv_min + (b * bucket_size) + (bucket_size / 2);
                let raise_target = (tokens_for_sale * bucket_fdv) / token_supply;

                if found == 0 && accumulated_demand >= raise_target {
                    clearing_bucket = b;
                    found = 1;
                }
            }
        }

        let clearing_fdv = fdv_min + (clearing_bucket * bucket_size) + (bucket_size / 2);
        let raise_target = (tokens_for_sale * clearing_fdv) / token_supply;

        // Compute demand strictly above clearing bucket
        let mut demand_above: u64 = 0;
        let mut demand_marginal: u64 = 0;
        for c in 0..MAX_BUCKET_COUNT {
            if (c as u64) < bucket_count {
                if c as u64 > clearing_bucket {
                    demand_above = demand_above + buckets[c];
                }
                if c as u64 == clearing_bucket {
                    demand_marginal = buckets[c];
                }
            }
        }

        // remaining_capacity for the marginal bucket
        let remaining_capacity = if raise_target > demand_above {
            raise_target - demand_above
        } else {
            0u64
        };

        let marginal_fill_rate_bps = if demand_marginal == 0 {
            10000u64
        } else if remaining_capacity >= demand_marginal {
            10000u64
        } else {
            (remaining_capacity * 10000) / demand_marginal
        };

        let total_raised = demand_above + if marginal_fill_rate_bps >= 10000 {
            demand_marginal
        } else {
            remaining_capacity
        };

        let clearing_bucket_low = fdv_min + (clearing_bucket * bucket_size);
        let clearing_bucket_high = clearing_bucket_low + bucket_size;

        (
            clearing_fdv.reveal(),
            marginal_fill_rate_bps.reveal(),
            clearing_bucket_low.reveal(),
            clearing_bucket_high.reveal(),
            total_raised.reveal(),
            bid_count.reveal(),
        )
    }

    #[instruction]
    fn check_bid_cleared(
        encrypted_max_fdv: Enc<Shared, u64>,
        clearing_bucket_low: u64,
        clearing_bucket_high: u64,
    ) -> u64 {
        let max_fdv = encrypted_max_fdv.to_arcis();
        let status = if max_fdv >= clearing_bucket_high {
            2u64
        } else if max_fdv >= clearing_bucket_low {
            1u64
        } else {
            0u64
        };
        status.reveal()
    }
}
