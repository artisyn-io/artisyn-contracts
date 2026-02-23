#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, IntoVal, Map, Symbol, Val,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_address.address()),
        StellarAssetClient::new(env, &contract_address.address()),
    )
}

fn setup_market_and_registry(
    env: &Env,
) -> (
    Address,
    MarketContractClient<'_>,
    Address,
    ::registry::RegistryClient<'_>,
) {
    let market_id = env.register(MarketContract, ());
    let market_client = MarketContractClient::new(env, &market_id);

    let registry_id = env.register(::registry::Registry, ());
    let registry_client = ::registry::RegistryClient::new(env, &registry_id);

    market_client.initialize(&registry_id);

    (market_id, market_client, registry_id, registry_client)
}

fn seed_artisan_profile(env: &Env, registry_id: &Address, artisan: &Address, role: u32) {
    env.as_contract(registry_id, || {
        use soroban_sdk::String;
        let profile = ::registry::Profile {
            role,
            metadata_hash: String::from_str(env, "hash"),
            is_verified: false,
        };
        env.storage()
            .persistent()
            .set(&::registry::DataKey::Profile(artisan.clone()), &profile);
    });
}

#[test]
fn test_create_job_transfers_funds_and_returns_id() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    // Mint tokens to finder
    token_admin_client.mint(&finder, &1000);
    assert_eq!(token_client.balance(&finder), 1000);

    // Create job — funds move, id starts at 1
    let job_id = client.create_job(&finder, &token_client.address, &500);
    assert_eq!(job_id, 1);
    assert_eq!(token_client.balance(&finder), 500);
    assert_eq!(token_client.balance(&contract_id), 500);

    // Second job increments counter
    let job_id_2 = client.create_job(&finder, &token_client.address, &200);
    assert_eq!(job_id_2, 2);
    assert_eq!(token_client.balance(&finder), 300);
    assert_eq!(token_client.balance(&contract_id), 700);
}

#[test]
fn test_create_job_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    client.create_job(&finder, &token_client.address, &500);

    // events().all() includes the token transfer event + our JobCreated event
    let all_events = env.events().all();
    assert_eq!(all_events.len(), 2);

    // The last event is our JobCreated event from the market contract
    // Wrap both sides in a soroban Vec so comparison uses host-level equality
    let expected_topics: soroban_sdk::Vec<Val> =
        vec![&env, Symbol::new(&env, "job_created").into_val(&env)];
    let expected_data: Val = Map::<Symbol, Val>::from_array(
        &env,
        [
            (Symbol::new(&env, "amount"), 500i128.into_val(&env)),
            (Symbol::new(&env, "id"), 1u64.into_val(&env)),
        ],
    )
    .into_val(&env);

    let expected: soroban_sdk::Vec<(Address, soroban_sdk::Vec<Val>, Val)> =
        vec![&env, (contract_id.clone(), expected_topics, expected_data)];
    assert_eq!(vec![&env, all_events.get(1).unwrap()], expected);
}

#[test]
fn test_create_job_incremental_ids() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &3000);

    assert_eq!(client.create_job(&finder, &token_client.address, &100), 1);
    assert_eq!(client.create_job(&finder, &token_client.address, &100), 2);
    assert_eq!(client.create_job(&finder, &token_client.address, &100), 3);
}

#[test]
fn test_assign_artisan_success() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup contracts
    let (market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    // Setup addresses
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    // Initialize registry
    registry_client.initialize(&admin);

    // Setup token
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Create artisan profile in registry (role = 3 for Artisan)
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    // Create a job
    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Assign artisan to job
    market_client.assign_artisan(&job_id, &artisan);

    // Verify JobAssigned event was emitted by checking events from market contract
    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();

    // Should have at least one event from market (JobAssigned)
    assert!(
        market_event_count >= 1,
        "Should have JobAssigned event from market contract"
    );
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_assign_artisan_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let artisan = Address::generate(&env);

    // Try to assign artisan to non-existent job
    market_client.assign_artisan(&999, &artisan);
}

#[test]
#[should_panic(expected = "Job is not open")]
fn test_assign_artisan_job_not_open() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Create artisan profile
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    // Assign artisan first time
    market_client.assign_artisan(&job_id, &artisan);

    // Try to assign again (job is now Assigned, not Open)
    let artisan2 = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &artisan2, 3);
    market_client.assign_artisan(&job_id, &artisan2);
}

#[test]
#[should_panic(expected = "User is not a verified Artisan")]
fn test_assign_artisan_not_verified() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let non_artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Create non-artisan profile (role = 0 for Finder)
    seed_artisan_profile(&env, &registry_id, &non_artisan, 0);

    // Try to assign non-artisan to job
    market_client.assign_artisan(&job_id, &non_artisan);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_assign_artisan_user_not_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let unregistered_user = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Try to assign unregistered user to job
    market_client.assign_artisan(&job_id, &unregistered_user);
}
