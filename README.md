# Artisyn.io Smart Contracts

## Find Artisans Near You

Artisyn is a decentralised protocol built on Stellar that connects local artisans with users through community-curated listings. Our platform creates a trustless ecosystem where skilled workers can be discovered, verified, and compensated securely without relying on centralised intermediaries.

Many artisans lack a platform to help them get noticed; meanwhile, numerous people would appreciate quality artisan recommendations—the kind we aim to provide. Our vision is to be the bridge connecting both worlds.

# Confirm Delivery Implementation Guide

## Overview
Implementation of the `confirm_delivery` function for the Artisyn Marketplace contract, allowing Finders to confirm work completion and release escrowed funds to Artisans.

---

## Files Created

1. **confirm_delivery_implementation.rs** - Main implementation
2. **confirm_delivery_tests.rs** - Comprehensive test suite

---

## Implementation Details

### Function Signature
```rust
pub fn confirm_delivery(env: Env, finder: Address, job_id: u64)
```

### Flow
1. ✅ Authenticate finder (`finder.require_auth()`)
2. ✅ Retrieve job and verify finder ownership
3. ✅ Validate job status is `PendingReview`
4. ✅ Calculate 1% platform fee
5. ✅ Transfer payout (99%) to artisan
6. ✅ Transfer fee (1%) to admin
7. ✅ Update job status to `Completed`
8. ✅ Emit `FundsReleased` event

### Fee Calculation
- **Platform Fee:** 1% of escrow amount
- **Artisan Payout:** 99% of escrow amount
- Uses integer arithmetic to avoid floating point: `(amount * 1) / 100`

---

## Integration Steps

### Step 1: Add to your lib.rs

Merge the implementation into your existing `contracts/market/src/lib.rs`:

```rust
// Add these imports at the top if not present
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec, log};

// Add the JobStatus enum if you don't have it
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobStatus {
    Created,
    InProgress,
    PendingReview,
    Completed,
    Disputed,
}

// Add the Job struct if you don't have it
#[derive(Clone)]
pub struct Job {
    pub id: u64,
    pub finder: Address,
    pub artisan: Address,
    pub escrow_amount: i128,
    pub status: JobStatus,
    pub description: String,
}

// Add the constants
const JOBS: Symbol = symbol_short!("JOBS");
const ADMIN: Symbol = symbol_short!("ADMIN");
const FEE_PERCENTAGE: u32 = 1;

// Then add the confirm_delivery function and helpers to your contractimpl block
```

### Step 2: Implement Token Transfer

The `transfer_funds` function is a placeholder. You need to implement actual token transfers:

**For Stellar Asset Contract:**
```rust
use soroban_sdk::token;

fn transfer_funds(env: &Env, recipient: &Address, amount: i128) {
    let token_address = get_token_address(env); // Your token address
    let token_client = token::Client::new(env, &token_address);
    
    token_client.transfer(
        &env.current_contract_address(),
        recipient,
        &amount
    );
}
```

**Or for custom token:**
```rust
fn transfer_funds(env: &Env, recipient: &Address, amount: i128) {
    // Call your custom token contract's transfer function
    let token_contract = YourTokenContractClient::new(env, &get_token_address(env));
    token_contract.transfer(&env.current_contract_address(), recipient, &amount);
}
```

### Step 3: Add Tests

Add the test file to `contracts/market/src/test.rs` or create it if it doesn't exist:

```rust
// In contracts/market/src/test.rs
#![cfg(test)]

mod tests {
    use super::*;
    // Include all test functions from confirm_delivery_tests.rs
}
```

### Step 4: Update Cargo.toml

Ensure you have the required dependencies:

```toml
[dependencies]
soroban-sdk = "20.0.0"  # Use your version

[dev-dependencies]
soroban-sdk = { version = "20.0.0", features = ["testutils"] }

[lib]
crate-type = ["cdylib"]
```

---

## Testing

### Run Tests
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_confirm_delivery_success

# Run with output
cargo test -- --nocapture

# Run in release mode
cargo test --release
```

### Test Coverage

The test suite includes:

✅ **Success Cases:**
- Basic delivery confirmation
- Large payment amounts
- Multiple job confirmations
- Event emission verification

✅ **Error Cases:**
- Unauthorized caller (not the finder)
- Wrong job status (Created, Completed, etc.)
- Non-existent job
- Invalid job ID

✅ **Edge Cases:**
- Fee calculation with various amounts
- Rounding behavior (amounts < 100)
- Large escrow amounts

---

## API Usage Examples

### JavaScript/TypeScript (using Soroban SDK)

```typescript
import { Contract, Address } from '@stellar/stellar-sdk';

// Initialize contract
const contract = new Contract(contractId);

// Confirm delivery
const finderAddress = "GABC...";
const jobId = 1;

const result = await contract.call(
  'confirm_delivery',
  finderAddress,
  jobId
);

console.log('Delivery confirmed!');
```

### Rust (calling from another contract)

```rust
use soroban_sdk::{Address, Env};

pub fn confirm_job_delivery(env: &Env, market_contract: Address, finder: Address, job_id: u64) {
    let market_client = MarketplaceContractClient::new(env, &market_contract);
    market_client.confirm_delivery(&finder, &job_id);
}
```

---

## Event Structure

### FundsReleased Event

```rust
// Event topics: (symbol_short!("FUNDS_REL"), job_id)
// Event data: (artisan_address, payout_amount)

// Example:
env.events().publish(
    (symbol_short!("FUNDS_REL"), 1u64),
    (artisan_address, 9_900i128)
);
```

**Listening for events:**
```typescript
// Get events from transaction
const events = result.events;
const fundsReleasedEvent = events.find(e => 
  e.topics[0] === 'FUNDS_REL'
);

console.log('Artisan:', fundsReleasedEvent.data[0]);
console.log('Payout:', fundsReleasedEvent.data[1]);
```

---

## Security Considerations

### ✅ Implemented Protections

1. **Authentication:** `finder.require_auth()` ensures only authorized caller
2. **Ownership Check:** Verifies caller is the job's finder
3. **Status Validation:** Only allows confirmation from `PendingReview` status
4. **Atomic Operations:** All transfers and state updates in single transaction
5. **Integer Arithmetic:** Avoids floating point for fee calculation

### ⚠️ Additional Recommendations

1. **Reentrancy Protection:** Consider adding reentrancy guards if needed
2. **Admin Address Validation:** Ensure admin address is always set
3. **Maximum Fee Cap:** Consider adding a maximum fee amount
4. **Pause Mechanism:** Add ability to pause contract in emergencies
5. **Upgrade Path:** Consider upgradeability for future changes

---

## Deployment Checklist

- [ ] Code review completed
- [ ] All tests passing
- [ ] Security audit performed
- [ ] Admin address configured
- [ ] Token contract integrated
- [ ] Fee percentage verified (1%)
- [ ] Event monitoring setup
- [ ] Documentation updated
- [ ] Integration tests with frontend
- [ ] Testnet deployment successful

---

## Troubleshooting

### Common Issues

**Issue:** "Admin address not set"
**Solution:** Initialize admin in contract initialization:
```rust
pub fn initialize(env: Env, admin: Address) {
    env.storage().instance().set(&ADMIN, &admin);
}
```

**Issue:** Token transfer fails
**Solution:** Ensure contract has approval to transfer tokens and sufficient balance

**Issue:** Job not found
**Solution:** Verify job was created and ID is correct

**Issue:** Tests fail to compile
**Solution:** Ensure `testutils` feature is enabled in dev-dependencies

---

## Next Steps

After implementing `confirm_delivery`, you might want to:

1. **Add Dispute Resolution:** Handle cases where finder/artisan disagree
2. **Implement Refunds:** Allow cancellation before PendingReview
3. **Add Escrow Extension:** Allow extending deadlines
4. **Multi-signature Approval:** Require multiple confirmations for large amounts
5. **Fee Tiers:** Different fees based on job amount or user tier

---

## Support

For issues or questions:
- GitHub Issues: http://github.com/artisyn-io/artisyn-contracts/issues
- Documentation: [Link to your docs]

---

## License

[Your License Here]

## Project

- 📱 [App](https://github.com/artisyn-io/artisyn.io)
- 📡 [Backend (API)](https://github.com/artisyn-io/artisyn-api)
- 📝 **[Smart Contracts (Current)](https://github.com/artisyn-io/artisyn-contracts)**
- [![Telegram](https://core.telegram.org/img/favicon-16x16.png) Telegram Channel](http://t.me/@artisynGF)

## Contribution Guide

To contribute to this project, check out the available issues, find one you can resolve, make something awesome and open a pull request.
