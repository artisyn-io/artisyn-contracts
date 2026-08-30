# Artisyn.io Smart Contracts

## Find Artisans Near You

Artisyn is a decentralised protocol built on Stellar that connects local artisans with users through community-curated listings. Our platform creates a trustless ecosystem where skilled workers can be discovered, verified, and compensated securely without relying on centralised intermediaries.

Many artisans lack a platform to help them get noticed; meanwhile, numerous people would appreciate quality artisan recommendations—the kind we aim to provide. Our vision is to be the bridge connecting both worlds.

## Project

- 📱 [App](https://github.com/artisyn-io/artisyn.io)
- 📡 [Backend (API)](https://github.com/artisyn-io/artisyn-api)
- 📝 **[Smart Contracts (Current)](https://github.com/artisyn-io/artisyn-contracts)**
- [![Telegram](https://core.telegram.org/img/favicon-16x16.png) Telegram Channel](http://t.me/@artisynGF)

## Gas Benchmarks

Resource usage and estimated fees for the two core market functions, measured
using the Soroban SDK's built-in invocation metering
(`env.cost_estimate().resources()` / `.fee()`).  Fee rates are based on a
Pubnet snapshot from 2024-12-11 (`fee_per_instruction_increment = 25 stroops /
10 000 instructions`).

> **Note on methodology:** Contracts run as native Rust inside the test
> environment, not as compiled WASM.  CPU instruction counts are therefore a
> **lower bound** — WASM execution adds VM instantiation and bytecode decoding
> overhead on top.  The `persistent_entry_rent` figures are also inflated in
> the test environment because the TTL extension of 500 000 ledgers is applied
> to brand-new entries starting from ledger 0; on Mainnet, where entries
> already carry an existing TTL, the incremental rent cost is far lower.  For
> exact pre-submission estimates, use `stellar contract invoke --cost` against a
> Testnet or Mainnet RPC endpoint.

### `create_job`

| Metric | Value |
|---|---|
| CPU instructions (native) | 305,208 |
| Memory bytes | 47,634 |
| In-memory read entries | 8 |
| Write entries | 5 |
| Write bytes | 1,516 |
| Contract events size | 380 bytes |

**Estimated resource fee breakdown (stroops):**

| Component | Stroops |
|---|---|
| Instructions | 764 |
| Read entries | 37,500 |
| Write entries | 50,000 |
| Write bytes | 5,182 |
| Contract events | 3,711 |
| **Execution subtotal** | **~97,157** |
| Persistent entry rent | _(inflated — see note above)_ |

Execution subtotal ≈ **97,157 stroops (~0.0097 XLM)**.

---

### `confirm_delivery`

| Metric | Value |
|---|---|
| CPU instructions (native) | 509,856 |
| Memory bytes | 80,404 |
| In-memory read entries | 10 |
| Write entries | 6 |
| Write bytes | 1,448 |
| Contract events size | 880 bytes |

**Estimated resource fee breakdown (stroops):**

| Component | Stroops |
|---|---|
| Instructions | 1,275 |
| Read entries | 43,750 |
| Write entries | 60,000 |
| Write bytes | 4,950 |
| Contract events | 8,594 |
| **Execution subtotal** | **~118,569** |
| Persistent entry rent | _(inflated — see note above)_ |

Execution subtotal ≈ **118,569 stroops (~0.0119 XLM)**.

---

### Reproducing the benchmarks

```bash
cargo test -p market --test bench -- --nocapture
```

The benchmark tests live in `contracts/market/tests/bench.rs`.

---

## Contribution Guide

To contribute to this project, check out the available issues, find one you can resolve, make something awesome and open a pull request.
