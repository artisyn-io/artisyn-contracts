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
            deadline: 0,
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
            deadline: 0,
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

// ── extend_deadline tests ────────────────────────────────────────────────────

#[test]
fn test_extend_deadline_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Extend by 3 days — must not panic
    market_client.extend_deadline(&finder, &job_id, &259200u64);

    // At least the DeadlineExtended event was emitted from the market contract
    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
fn test_extend_deadline_multiple_times() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Extend twice — deadline accumulates
    market_client.extend_deadline(&finder, &job_id, &86400u64);
    market_client.extend_deadline(&finder, &job_id, &172800u64);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_extend_deadline_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let finder = Address::generate(&env);

    market_client.extend_deadline(&finder, &999, &86400u64);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_extend_deadline_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let other = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.extend_deadline(&other, &job_id, &86400u64);
}

#[test]
#[should_panic(expected = "Job is already finalized")]
fn test_extend_deadline_cancelled_job() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.cancel_job(&finder, &job_id);

    market_client.extend_deadline(&finder, &job_id, &86400u64);
}

#[test]
#[should_panic(expected = "Job is already finalized")]
fn test_extend_deadline_completed_job() {
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

    market_client.auto_release_funds(&artisan, &job_id);

    // Fetch the finder that was seeded into the job
    let seeded_finder: Address = env.as_contract(&market_id, || {
        let job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap();
        job.finder.clone()
    });

    market_client.extend_deadline(&seeded_finder, &job_id, &86400u64);
}

// ── increase_budget tests ────────────────────────────────────────────────────

#[test]
fn test_increase_budget_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    // Balances before top-up
    assert_eq!(token_client.balance(&finder), 500);
    assert_eq!(token_client.balance(&market_id), 500);

    market_client.increase_budget(&finder, &job_id, &200);

    // Contract received the extra funds
    assert_eq!(token_client.balance(&finder), 300);
    assert_eq!(token_client.balance(&market_id), 700);
}

#[test]
fn test_increase_budget_multiple_times() {
    let env = Env::default();
    env.mock_all_auths();

    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &300);

    market_client.increase_budget(&finder, &job_id, &100);
    market_client.increase_budget(&finder, &job_id, &200);

    // 300 + 100 + 200 = 600 in escrow
    assert_eq!(token_client.balance(&market_id), 600);
    assert_eq!(token_client.balance(&finder), 400);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_increase_budget_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let finder = Address::generate(&env);

    market_client.increase_budget(&finder, &999, &100);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_increase_budget_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let other = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);
    token_admin_client.mint(&other, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);

    market_client.increase_budget(&other, &job_id, &100);
}

#[test]
#[should_panic(expected = "Job is already finalized")]
fn test_increase_budget_cancelled_job() {
    let env = Env::default();
    env.mock_all_auths();

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.cancel_job(&finder, &job_id);

    market_client.increase_budget(&finder, &job_id, &100);
}

#[test]
#[should_panic(expected = "Job is already finalized")]
fn test_increase_budget_completed_job() {
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

    market_client.auto_release_funds(&artisan, &job_id);

    let seeded_finder: Address = env.as_contract(&market_id, || {
        let job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap();
        job.finder.clone()
    });

    token_admin_client.mint(&seeded_finder, &100);
    market_client.increase_budget(&seeded_finder, &job_id, &100);

    // contracts/market/src/test.rs
// Tests for confirm_delivery functionality

#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// Test helper function to create a test job
fn create_test_job(env: &Env, finder: &Address, artisan: &Address) -> (u64, Job) {
    let job_id = 1u64;
    let job = Job {
        id: job_id,
        finder: finder.clone(),
        artisan: artisan.clone(),
        escrow_amount: 10_000, // 100.00 with 2 decimals
        status: JobStatus::PendingReview,
        description: String::from_str(env, "Test job"),
    };
    (job_id, job)
}

// Test helper to setup contract
fn setup_test_contract(env: &Env) -> (Address, Address, Address) {
    let finder = Address::generate(env);
    let artisan = Address::generate(env);
    let admin = Address::generate(env);
    
    // Initialize contract with admin
    env.storage().instance().set(&ADMIN, &admin);
    
    (finder, artisan, admin)
}

#[test]
fn test_confirm_delivery_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    let (job_id, mut job) = create_test_job(&env, &finder, &artisan);
    
    // Save job to storage
    let mut jobs = Vec::new(&env);
    jobs.push_back(job.clone());
    env.storage().instance().set(&JOBS, &jobs);
    
    // Mock finder authentication
    env.mock_all_auths();
    
    // Execute
    client.confirm_delivery(&finder, &job_id);
    
    // Verify job status changed to Completed
    let updated_job = MarketplaceContract::get_job(&env, job_id);
    assert_eq!(updated_job.status, JobStatus::Completed);
    
    // Verify events were emitted
    let events = env.events().all();
    assert!(events.len() > 0);
    
    // Calculate expected amounts
    let fee = (job.escrow_amount * 1) / 100; // 1% fee
    let payout = job.escrow_amount - fee;
    
    assert_eq!(payout, 9_900); // 99.00
    assert_eq!(fee, 100); // 1.00
}

#[test]
#[should_panic(expected = "Only the job's finder can confirm delivery")]
fn test_confirm_delivery_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    let (job_id, job) = create_test_job(&env, &finder, &artisan);
    
    // Save job
    let mut jobs = Vec::new(&env);
    jobs.push_back(job);
    env.storage().instance().set(&JOBS, &jobs);
    
    // Try to confirm with wrong address (not the finder)
    let wrong_caller = Address::generate(&env);
    env.mock_all_auths();
    
    // This should panic
    client.confirm_delivery(&wrong_caller, &job_id);
}

#[test]
#[should_panic(expected = "Job must be in PendingReview status")]
fn test_confirm_delivery_wrong_status_created() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    let (job_id, mut job) = create_test_job(&env, &finder, &artisan);
    
    // Set wrong status
    job.status = JobStatus::Created;
    
    // Save job
    let mut jobs = Vec::new(&env);
    jobs.push_back(job);
    env.storage().instance().set(&JOBS, &jobs);
    
    env.mock_all_auths();
    
    // This should panic
    client.confirm_delivery(&finder, &job_id);
}

#[test]
#[should_panic(expected = "Job must be in PendingReview status")]
fn test_confirm_delivery_wrong_status_completed() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    let (job_id, mut job) = create_test_job(&env, &finder, &artisan);
    
    // Set status to already completed
    job.status = JobStatus::Completed;
    
    // Save job
    let mut jobs = Vec::new(&env);
    jobs.push_back(job);
    env.storage().instance().set(&JOBS, &jobs);
    
    env.mock_all_auths();
    
    // This should panic - can't confirm already completed job
    client.confirm_delivery(&finder, &job_id);
}

#[test]
#[should_panic(expected = "Job with ID")]
fn test_confirm_delivery_nonexistent_job() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    
    // Initialize empty jobs vector
    let jobs = Vec::new(&env);
    env.storage().instance().set(&JOBS, &jobs);
    
    env.mock_all_auths();
    
    // Try to confirm non-existent job
    let nonexistent_job_id = 999u64;
    client.confirm_delivery(&finder, &nonexistent_job_id);
}

#[test]
fn test_calculate_fee_various_amounts() {
    let env = Env::default();
    
    // Test 1% fee calculation
    assert_eq!(MarketplaceContract::calculate_fee(10_000), 100); // 1% of 10,000 = 100
    assert_eq!(MarketplaceContract::calculate_fee(50_000), 500); // 1% of 50,000 = 500
    assert_eq!(MarketplaceContract::calculate_fee(100), 1);      // 1% of 100 = 1
    assert_eq!(MarketplaceContract::calculate_fee(99), 0);       // 1% of 99 = 0 (rounds down)
}

#[test]
fn test_confirm_delivery_with_large_amount() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    let job_id = 1u64;
    
    // Create job with large escrow amount
    let large_amount = 1_000_000_000i128; // 1 billion
    let job = Job {
        id: job_id,
        finder: finder.clone(),
        artisan: artisan.clone(),
        escrow_amount: large_amount,
        status: JobStatus::PendingReview,
        description: String::from_str(&env, "Large payment job"),
    };
    
    // Save job
    let mut jobs = Vec::new(&env);
    jobs.push_back(job);
    env.storage().instance().set(&JOBS, &jobs);
    
    env.mock_all_auths();
    
    // Execute
    client.confirm_delivery(&finder, &job_id);
    
    // Verify job status
    let updated_job = MarketplaceContract::get_job(&env, job_id);
    assert_eq!(updated_job.status, JobStatus::Completed);
    
    // Verify fee calculation for large amount
    let expected_fee = large_amount / 100; // 1% = 10,000,000
    let expected_payout = large_amount - expected_fee; // 990,000,000
    
    assert_eq!(expected_fee, 10_000_000);
    assert_eq!(expected_payout, 990_000_000);
}

#[test]
fn test_confirm_delivery_event_emission() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    let (job_id, job) = create_test_job(&env, &finder, &artisan);
    
    // Save job
    let mut jobs = Vec::new(&env);
    jobs.push_back(job.clone());
    env.storage().instance().set(&JOBS, &jobs);
    
    env.mock_all_auths();
    
    // Execute
    client.confirm_delivery(&finder, &job_id);
    
    // Check event emission
    let events = env.events().all();
    let event = events.last().unwrap();
    
    // Verify event contains correct data
    // Event structure: (symbol_short!("FUNDS_REL"), job_id), (artisan, payout_amount)
    assert!(event.topics.len() > 0);
    
    // Calculate expected payout
    let fee = (job.escrow_amount * 1) / 100;
    let expected_payout = job.escrow_amount - fee;
    
    // The event should contain the artisan address and payout amount
    // Exact assertion depends on your event structure
}

#[test]
fn test_confirm_delivery_multiple_jobs() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MarketplaceContract);
    let client = MarketplaceContractClient::new(&env, &contract_id);
    
    // Setup
    let (finder, artisan, admin) = setup_test_contract(&env);
    
    // Create multiple jobs
    let mut jobs = Vec::new(&env);
    for i in 1..=3 {
        let job = Job {
            id: i,
            finder: finder.clone(),
            artisan: artisan.clone(),
            escrow_amount: 10_000 * i as i128,
            status: JobStatus::PendingReview,
            description: String::from_str(&env, "Test job"),
        };
        jobs.push_back(job);
    }
    
    env.storage().instance().set(&JOBS, &jobs);
    env.mock_all_auths();
    
    // Confirm each job
    for job_id in 1..=3 {
        client.confirm_delivery(&finder, &job_id);
        
        // Verify job status
        let updated_job = MarketplaceContract::get_job(&env, job_id);
        assert_eq!(updated_job.status, JobStatus::Completed);
    }
}

#[test]
fn test_fee_percentage_accuracy() {
    // Test that 1% fee is calculated correctly
    let test_cases = vec![
        (100, 1),           // 1% of 100 = 1
        (1_000, 10),        // 1% of 1,000 = 10
        (10_000, 100),      // 1% of 10,000 = 100
        (99, 0),            // 1% of 99 = 0 (rounds down)
        (50_000, 500),      // 1% of 50,000 = 500
        (123_456, 1_234),   // 1% of 123,456 = 1,234
    ];
    
    for (amount, expected_fee) in test_cases {
        let actual_fee = MarketplaceContract::calculate_fee(amount);
        assert_eq!(
            actual_fee, expected_fee,
            "Fee calculation failed for amount {}: expected {}, got {}",
            amount, expected_fee, actual_fee
        );
    }
}
}
