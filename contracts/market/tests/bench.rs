//! Gas / Resource Benchmarks for the Market contract.
//!
//! This integration test lives in a separate crate (under `tests/`) so it has
//! access to `std` even though the contract library itself is `#![no_std]`.
//!
//! It uses the Soroban SDK's built-in invocation metering to measure CPU
//! instructions, memory bytes, and estimated resource fees (in stroops) for
//! the two core market functions: `create_job` and `confirm_delivery`.
//!
//! # Methodology
//!
//! * `Env::default()` automatically enables `InvocationMeter` in test mode.
//! * `env.cost_estimate().resources()` returns per-invocation resource usage
//!   measured immediately after each contract call.  The meter resets before
//!   every top-level contract invocation, so only the targeted call is counted.
//! * `env.cost_estimate().fee()` converts those resources into stroop estimates
//!   using a snapshot of Pubnet fee rates taken on 2024-12-11.
//! * Because contracts run as native Rust inside the test environment (not as
//!   compiled WASM), CPU instruction counts are a lower bound.  On the real
//!   network WASM execution adds VM overhead (instantiation, bytecode decoding,
//!   etc.) on top.  Use these numbers for relative comparison and cost
//!   budgeting; for exact pre-submission estimates use `stellar contract invoke
//!   --cost` against a Testnet/Mainnet RPC endpoint.
//!
//! # Running
//!
//! ```bash
//! cargo test -p market --test bench -- --nocapture
//! ```

use market::{MarketContract, MarketContractClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn create_token<'a>(env: &'a Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let addr = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &addr.address()),
        StellarAssetClient::new(env, &addr.address()),
    )
}

fn setup_market_and_registry<'a>(
    env: &'a Env,
    admin: &Address,
) -> (
    Address,
    MarketContractClient<'a>,
    Address,
    registry::RegistryClient<'a>,
) {
    let market_id = env.register(MarketContract, ());
    let market_client = MarketContractClient::new(env, &market_id);
    let registry_id = env.register(registry::Registry, ());
    let registry_client = registry::RegistryClient::new(env, &registry_id);
    market_client.initialize(&registry_id, admin);
    (market_id, market_client, registry_id, registry_client)
}

fn seed_artisan(env: &Env, registry_id: &Address, artisan: &Address) {
    env.as_contract(registry_id, || {
        use soroban_sdk::String;
        let profile = registry::Profile {
            role: 3,
            metadata_hash: String::from_str(env, "hash"),
            is_verified: false,
            is_blacklisted: false,
        };
        env.storage()
            .persistent()
            .set(&registry::DataKey::Profile(artisan.clone()), &profile);
    });
}

// ---------------------------------------------------------------------------
// Benchmark: create_job
// ---------------------------------------------------------------------------

/// Profiles the resource footprint of a single `create_job` invocation.
///
/// The environment is fully initialised before the measurement so that only
/// the work done by `create_job` itself is captured.
#[test]
fn bench_create_job() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, _, _) = setup_market_and_registry(&env, &admin);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1_000_000);

    // ── measured call ────────────────────────────────────────────────────────
    let job_id = market_client.create_job(&finder, &token_client.address, &500_000, &0);
    // ────────────────────────────────────────────────────────────────────────

    let resources = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    let budget = env.cost_estimate().budget();

    println!("\n╔═══════════════════════════════════════════════╗");
    println!("║        BENCHMARK: create_job                  ║");
    println!("╠═══════════════════════════════════════════════╣");
    println!("║ Resource usage (InvocationResources):         ║");
    println!(
        "║   CPU instructions       : {:>12}       ║",
        resources.instructions
    );
    println!(
        "║   Memory bytes           : {:>12}       ║",
        resources.mem_bytes
    );
    println!(
        "║   In-memory read entries : {:>12}       ║",
        resources.memory_read_entries
    );
    println!(
        "║   Write entries          : {:>12}       ║",
        resources.write_entries
    );
    println!(
        "║   Write bytes            : {:>12}       ║",
        resources.write_bytes
    );
    println!(
        "║   Events size bytes      : {:>12}       ║",
        resources.contract_events_size_bytes
    );
    println!(
        "║   Persistent rent lb     : {:>12}       ║",
        resources.persistent_rent_ledger_bytes
    );
    println!("╠═══════════════════════════════════════════════╣");
    println!("║ Fee estimate — Pubnet rates (2024-12-11):     ║");
    println!(
        "║   instructions fee       : {:>10} stroops ║",
        fee.instructions
    );
    println!(
        "║   read entries fee       : {:>10} stroops ║",
        fee.disk_read_entries
    );
    println!(
        "║   write entries fee      : {:>10} stroops ║",
        fee.write_entries
    );
    println!(
        "║   write bytes fee        : {:>10} stroops ║",
        fee.write_bytes
    );
    println!(
        "║   events fee             : {:>10} stroops ║",
        fee.contract_events
    );
    println!(
        "║   persistent rent        : {:>10} stroops ║",
        fee.persistent_entry_rent
    );
    println!("║ ─────────────────────────────────────────── ║");
    println!("║   TOTAL resource fee     : {:>10} stroops ║", fee.total);
    println!("╠═══════════════════════════════════════════════╣");
    println!("║ Budget totals (native Rust — lower bound):    ║");
    println!(
        "║   budget cpu insns       : {:>12}       ║",
        budget.cpu_instruction_cost()
    );
    println!(
        "║   budget mem bytes       : {:>12}       ║",
        budget.memory_bytes_cost()
    );
    println!("╚═══════════════════════════════════════════════╝");

    // Correctness checks
    assert_eq!(job_id, 1, "first job must have id 1");
    assert!(resources.instructions > 0, "instructions must be non-zero");
    assert!(
        resources.write_entries >= 2,
        "must write at least job entry + counter (got {})",
        resources.write_entries
    );
    assert!(fee.total > 0, "total fee estimate must be positive");
}

// ---------------------------------------------------------------------------
// Benchmark: confirm_delivery
// ---------------------------------------------------------------------------

/// Profiles the resource footprint of a single `confirm_delivery` invocation.
///
/// All prerequisite state (create_job → apply_for_job → assign_artisan → start_job →
/// complete_job) is set up before the measurement so that only
/// `confirm_delivery` itself is metered.
#[test]
fn bench_confirm_delivery() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) = setup_market_and_registry(&env, &admin);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1_000_000);
    seed_artisan(&env, &registry_id, &artisan);

    // Build pre-conditions. Each call resets the invocation meter, so these
    // are NOT included in the measurement below.
    let job_id = market_client.create_job(&finder, &token_client.address, &500_000, &0);
    market_client.apply_for_job(&artisan, &job_id);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    // ── measured call ────────────────────────────────────────────────────────
    market_client.confirm_delivery(&finder, &job_id);
    // ────────────────────────────────────────────────────────────────────────

    let resources = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    let budget = env.cost_estimate().budget();

    println!("\n╔═══════════════════════════════════════════════╗");
    println!("║     BENCHMARK: confirm_delivery               ║");
    println!("╠═══════════════════════════════════════════════╣");
    println!("║ Resource usage (InvocationResources):         ║");
    println!(
        "║   CPU instructions       : {:>12}       ║",
        resources.instructions
    );
    println!(
        "║   Memory bytes           : {:>12}       ║",
        resources.mem_bytes
    );
    println!(
        "║   In-memory read entries : {:>12}       ║",
        resources.memory_read_entries
    );
    println!(
        "║   Write entries          : {:>12}       ║",
        resources.write_entries
    );
    println!(
        "║   Write bytes            : {:>12}       ║",
        resources.write_bytes
    );
    println!(
        "║   Events size bytes      : {:>12}       ║",
        resources.contract_events_size_bytes
    );
    println!(
        "║   Persistent rent lb     : {:>12}       ║",
        resources.persistent_rent_ledger_bytes
    );
    println!("╠═══════════════════════════════════════════════╣");
    println!("║ Fee estimate — Pubnet rates (2024-12-11):     ║");
    println!(
        "║   instructions fee       : {:>10} stroops ║",
        fee.instructions
    );
    println!(
        "║   read entries fee       : {:>10} stroops ║",
        fee.disk_read_entries
    );
    println!(
        "║   write entries fee      : {:>10} stroops ║",
        fee.write_entries
    );
    println!(
        "║   write bytes fee        : {:>10} stroops ║",
        fee.write_bytes
    );
    println!(
        "║   events fee             : {:>10} stroops ║",
        fee.contract_events
    );
    println!(
        "║   persistent rent        : {:>10} stroops ║",
        fee.persistent_entry_rent
    );
    println!("║ ─────────────────────────────────────────── ║");
    println!("║   TOTAL resource fee     : {:>10} stroops ║", fee.total);
    println!("╠═══════════════════════════════════════════════╣");
    println!("║ Budget totals (native Rust — lower bound):    ║");
    println!(
        "║   budget cpu insns       : {:>12}       ║",
        budget.cpu_instruction_cost()
    );
    println!(
        "║   budget mem bytes       : {:>12}       ║",
        budget.memory_bytes_cost()
    );
    println!("╚═══════════════════════════════════════════════╝");

    // Correctness checks: 1% fee on 500_000 → 5_000 fee, 495_000 to artisan
    let expected_fee_amount = 500_000i128 * 100 / 10_000; // 5_000
    let expected_payout = 500_000i128 - expected_fee_amount; // 495_000
    assert_eq!(
        token_client.balance(&artisan),
        expected_payout,
        "artisan should receive {} (got {})",
        expected_payout,
        token_client.balance(&artisan)
    );
    assert_eq!(
        token_client.balance(&admin),
        expected_fee_amount,
        "admin should receive platform fee"
    );
    assert!(resources.instructions > 0, "instructions must be non-zero");
    assert!(fee.total > 0, "total fee estimate must be positive");
}
