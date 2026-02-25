use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
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
            is_blacklisted: false,
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

    token_admin_client.mint(&finder, &1000);
    assert_eq!(token_client.balance(&finder), 1000);

    let job_id = client.create_job(&finder, &token_client.address, &500);
    assert_eq!(job_id, 1);
    assert_eq!(token_client.balance(&finder), 500);
    assert_eq!(token_client.balance(&contract_id), 500);
}

#[test]
fn test_assign_artisan_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.assign_artisan(&finder, &job_id, &artisan);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_assign_artisan_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    market_client.assign_artisan(&finder, &999, &artisan);
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
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    market_client.assign_artisan(&finder, &job_id, &artisan);

    let artisan2 = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &artisan2, 3);
    market_client.assign_artisan(&finder, &job_id, &artisan2);
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

    seed_artisan_profile(&env, &registry_id, &non_artisan, 0);

    market_client.assign_artisan(&finder, &job_id, &non_artisan);
}

#[test]
fn test_apply_for_job_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.apply_for_job(&artisan, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_apply_for_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);
    let artisan = Address::generate(&env);

    market_client.apply_for_job(&artisan, &999);
}

#[test]
#[should_panic(expected = "Job is not open")]
fn test_apply_for_job_not_open() {
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
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    market_client.assign_artisan(&finder, &job_id, &artisan);

    let artisan2 = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &artisan2, 3);
    market_client.apply_for_job(&artisan2, &job_id);
}

#[test]
#[should_panic(expected = "User is not a verified Artisan")]
fn test_apply_for_job_not_artisan() {
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

    seed_artisan_profile(&env, &registry_id, &non_artisan, 0);

    market_client.apply_for_job(&non_artisan, &job_id);
}

#[test]
#[should_panic(expected = "User is blacklisted")]
fn test_apply_for_job_blacklisted() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let blacklisted_artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    env.as_contract(&registry_id, || {
        use soroban_sdk::String;
        let profile = ::registry::Profile {
            role: 3,
            metadata_hash: String::from_str(&env, "hash"),
            is_verified: false,
            is_blacklisted: true,
        };
        env.storage().persistent().set(
            &::registry::DataKey::Profile(blacklisted_artisan.clone()),
            &profile,
        );
    });

    market_client.apply_for_job(&blacklisted_artisan, &job_id);
}

#[test]
fn test_start_job_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    market_client.start_job(&artisan, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_start_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);
    let artisan = Address::generate(&env);

    market_client.start_job(&artisan, &999);
}

#[test]
#[should_panic(expected = "Not assigned to this job")]
fn test_start_job_not_assigned() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let wrong_artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    market_client.start_job(&wrong_artisan, &job_id);
}

#[test]
#[should_panic(expected = "Job is not assigned")]
fn test_start_job_wrong_status() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.start_job(&artisan, &job_id);
}

#[test]
#[should_panic(expected = "Job is not assigned")]
fn test_start_job_already_started() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    market_client.start_job(&artisan, &job_id);
}

#[test]
fn test_cancel_job_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _, _) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    let finder_balance_before = token_client.balance(&finder);
    let contract_balance_before = token_client.balance(&market_id);

    market_client.cancel_job(&finder, &job_id);

    let finder_balance_after = token_client.balance(&finder);
    let contract_balance_after = token_client.balance(&market_id);

    assert_eq!(finder_balance_after, finder_balance_before + 500);
    assert_eq!(contract_balance_after, contract_balance_before - 500);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_cancel_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, market_client, _, _) = setup_market_and_registry(&env);

    let finder = Address::generate(&env);

    market_client.cancel_job(&finder, &999);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_cancel_job_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let market_client = setup_market_and_registry(&env).1;

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let other_user = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.cancel_job(&other_user, &job_id);
}

#[test]
#[should_panic(expected = "Job is not open")]
fn test_cancel_job_already_assigned() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, market_client, registry_id, _) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.assign_artisan(&finder, &job_id, &artisan);

    market_client.cancel_job(&finder, &job_id);
}

#[test]
#[should_panic(expected = "Job is not open")]
fn test_cancel_job_already_in_progress() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, market_client, registry_id, _) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    market_client.cancel_job(&finder, &job_id);
}

#[test]
fn test_complete_job_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    market_client.complete_job(&artisan, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_complete_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, market_client, _, _) = setup_market_and_registry(&env);
    let artisan = Address::generate(&env);

    market_client.complete_job(&artisan, &999);
}

#[test]
#[should_panic(expected = "Not assigned to this job")]
fn test_complete_job_not_assigned() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let wrong_artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    market_client.complete_job(&wrong_artisan, &job_id);
}

#[test]
#[should_panic(expected = "Job is not in progress")]
fn test_complete_job_wrong_status() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, market_client, registry_id, registry_client) = setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    // Job is assigned, but not started yet
    market_client.complete_job(&artisan, &job_id);
}
