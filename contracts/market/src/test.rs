use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
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

fn create_job_in_pending_review(
    env: &Env,
    market_id: &Address,
    artisan: &Address,
    token_address: &Address,
    amount: i128,
    end_time: u64,
) -> u64 {
    env.as_contract(market_id, || {
        let job_id = 1u64;
        let job = Job {
            id: job_id,
            finder: Address::generate(env),
            artisan: Some(artisan.clone()),
            token: token_address.clone(),
            amount,
            status: JobStatus::PendingReview,
            start_time: 0,
            end_time,
        };
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage().instance().set(&DataKey::JobCounter, &job_id);
        job_id
    })
}

#[test]
fn test_auto_release_funds_success_after_7_days() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let artisan = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    token_admin_client.mint(&market_id, &500);

    let end_time = 1000u64;

    let job_id = create_job_in_pending_review(
        &env,
        &market_id,
        &artisan,
        &token_client.address,
        500,
        end_time,
    );

    env.ledger().with_mut(|li| {
        li.timestamp = end_time + 604800 + 1;
    });

    assert_eq!(token_client.balance(&artisan), 0);
    assert_eq!(token_client.balance(&market_id), 500);

    market_client.auto_release_funds(&artisan, &job_id);

    assert_eq!(token_client.balance(&artisan), 500);
    assert_eq!(token_client.balance(&market_id), 0);
}

#[test]
#[should_panic(expected = "7 days have not passed since job completion")]
fn test_auto_release_funds_fails_before_7_days() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let artisan = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    token_admin_client.mint(&market_id, &500);

    let end_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = end_time + 100;
    });

    let job_id = create_job_in_pending_review(
        &env,
        &market_id,
        &artisan,
        &token_client.address,
        500,
        end_time,
    );

    market_client.auto_release_funds(&artisan, &job_id);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_auto_release_funds_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let artisan = Address::generate(&env);

    market_client.auto_release_funds(&artisan, &999);
}

#[test]
#[should_panic(expected = "Job is not in PendingReview status")]
fn test_auto_release_funds_wrong_status() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let artisan = Address::generate(&env);
    let (token_client, _token_admin_client) = create_token(&env, &admin);

    env.as_contract(&market_id, || {
        let job_id = 1u64;
        let job = Job {
            id: job_id,
            finder: Address::generate(&env),
            artisan: Some(artisan.clone()),
            token: token_client.address.clone(),
            amount: 500,
            status: JobStatus::Completed,
            start_time: 0,
            end_time: 1000,
        };
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
    });

    market_client.auto_release_funds(&artisan, &1);
}

#[test]
#[should_panic(expected = "Only the assigned artisan can release funds")]
fn test_auto_release_funds_wrong_artisan() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let artisan = Address::generate(&env);
    let wrong_artisan = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    token_admin_client.mint(&market_id, &500);

    let end_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = end_time + 604800 + 1;
    });

    let job_id = create_job_in_pending_review(
        &env,
        &market_id,
        &artisan,
        &token_client.address,
        500,
        end_time,
    );

    market_client.auto_release_funds(&wrong_artisan, &job_id);
}
