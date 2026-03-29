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
    admin: Address,
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

    market_client.initialize(&registry_id, &admin);

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

    let admin = Address::generate(&env);
    let (contract_id, client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

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
    let admin = Address::generate(&env);

    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin);
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    market_client.assign_artisan(&finder, &999, &artisan);
}

#[test]
#[should_panic(expected = "Job is not open")]
fn test_assign_artisan_job_not_open() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);

    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin);
    let artisan = Address::generate(&env);

    market_client.apply_for_job(&artisan, &999);
}

#[test]
#[should_panic(expected = "Job is not open")]
fn test_apply_for_job_not_open() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let artisan = Address::generate(&env);

    market_client.start_job(&artisan, &999);
}

#[test]
#[should_panic(expected = "Not assigned to this job")]
fn test_start_job_not_assigned() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (market_id, market_client, _, _) = setup_market_and_registry(&env, admin.clone());

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

    let admin = Address::generate(&env);
    let (_, market_client, _, _) = setup_market_and_registry(&env, admin);

    let finder = Address::generate(&env);

    market_client.cancel_job(&finder, &999);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_cancel_job_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let market_client = setup_market_and_registry(&env, admin).1;

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

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, _) = setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, _) = setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_, market_client, _, _) = setup_market_and_registry(&env, admin);
    let artisan = Address::generate(&env);

    market_client.complete_job(&artisan, &999);
}

#[test]
#[should_panic(expected = "Not assigned to this job")]
fn test_complete_job_not_assigned() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

#[test]
fn test_confirm_delivery_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    assert_eq!(token_client.balance(&market_id), 500);
    assert_eq!(token_client.balance(&artisan), 0);
    assert_eq!(token_client.balance(&admin), 0);

    market_client.confirm_delivery(&finder, &job_id);

    // 1% fee on 500 => 5 to admin, 495 to artisan
    assert_eq!(token_client.balance(&artisan), 495);
    assert_eq!(token_client.balance(&admin), 5);
    assert_eq!(token_client.balance(&market_id), 0);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.status, JobStatus::Completed);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_confirm_delivery_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, _, _) = setup_market_and_registry(&env, admin);
    let finder = Address::generate(&env);

    market_client.confirm_delivery(&finder, &999);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_confirm_delivery_not_finder() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let other = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    market_client.confirm_delivery(&other, &job_id);
}

#[test]
#[should_panic(expected = "Job is not pending review")]
fn test_confirm_delivery_wrong_status() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    market_client.confirm_delivery(&finder, &job_id);
}

#[test]
fn test_raise_dispute_success_from_in_progress_by_finder() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    market_client.raise_dispute(&finder, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.status, JobStatus::Disputed);
}

#[test]
fn test_raise_dispute_success_from_pending_review_by_artisan() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    market_client.raise_dispute(&artisan, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.status, JobStatus::Disputed);
}

#[test]
#[should_panic(expected = "Only the finder or assigned artisan can raise a dispute")]
fn test_raise_dispute_unauthorized_user() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let random_user = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    market_client.raise_dispute(&random_user, &job_id);
}

#[test]
#[should_panic(expected = "Job cannot be disputed in its current status")]
fn test_raise_dispute_wrong_status() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    market_client.raise_dispute(&finder, &job_id);
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
            juror: None,
            token: token_address.clone(),
            amount,
            status: JobStatus::PendingReview,
            start_time: 0,
            end_time,
            deadline: 0,
            dispute_reason: None,
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin);

    let artisan = Address::generate(&env);

    market_client.auto_release_funds(&artisan, &999);
}

#[test]
#[should_panic(expected = "Job is not in PendingReview status")]
fn test_auto_release_funds_wrong_status() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let artisan = Address::generate(&env);
    let (token_client, _token_admin_client) = create_token(&env, &admin);

    env.as_contract(&market_id, || {
        let job_id = 1u64;
        let job = Job {
            id: job_id,
            finder: Address::generate(&env),
            artisan: Some(artisan.clone()),
            juror: None,
            token: token_client.address.clone(),
            amount: 500,
            status: JobStatus::Completed,
            start_time: 0,
            end_time: 1000,
            deadline: 0,
            dispute_reason: None,
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin);

    let finder = Address::generate(&env);

    market_client.extend_deadline(&finder, &999, &86400u64);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_extend_deadline_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin);

    let finder = Address::generate(&env);

    market_client.increase_budget(&finder, &999, &100);
}

#[test]
#[should_panic(expected = "Not job owner")]
fn test_increase_budget_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
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
}

// ── transfer_admin tests ─────────────────────────────────────────────────────

#[test]
fn test_transfer_admin_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.transfer_admin(&admin, &new_admin);

    // Verify new admin can transfer again (old admin can no longer)
    let another_admin = Address::generate(&env);
    market_client.transfer_admin(&new_admin, &another_admin);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_transfer_admin_wrong_caller() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.transfer_admin(&impostor, &new_admin);
}

#[test]
#[should_panic(expected = "Missing storage variable")]
fn test_transfer_admin_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.transfer_admin(&admin, &new_admin);
}

// ── toggle_contract_pause tests ──────────────────────────────────────────────

#[test]
fn test_toggle_contract_pause_pauses() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.toggle_contract_pause(&admin);

    // Verify IsPaused is now true via storage inspection
    let is_paused: bool = env.as_contract(&market_id, || {
        env.storage().instance().get(&DataKey::IsPaused).unwrap()
    });
    assert!(is_paused);
}

#[test]
fn test_toggle_contract_pause_unpauses() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    // Pause then unpause
    market_client.toggle_contract_pause(&admin);
    market_client.toggle_contract_pause(&admin);

    let is_paused: bool = env.as_contract(&market_id, || {
        env.storage().instance().get(&DataKey::IsPaused).unwrap()
    });
    assert!(!is_paused);
}

#[test]
fn test_toggle_contract_pause_multiple_times() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    for expected in [true, false, true, false] {
        market_client.toggle_contract_pause(&admin);
        let is_paused: bool = env.as_contract(&market_id, || {
            env.storage().instance().get(&DataKey::IsPaused).unwrap()
        });
        assert_eq!(is_paused, expected);
    }
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_toggle_contract_pause_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.toggle_contract_pause(&impostor);
}

#[test]
#[should_panic(expected = "Admin not set")]
fn test_toggle_contract_pause_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.toggle_contract_pause(&admin);
}

// ── pause-gated function tests ───────────────────────────────────────────────

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_create_job_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    market_client.toggle_contract_pause(&admin);
    market_client.create_job(&finder, &token_client.address, &500);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_assign_artisan_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.assign_artisan(&finder, &1, &artisan);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_apply_for_job_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let artisan = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.apply_for_job(&artisan, &1);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_start_job_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let artisan = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.start_job(&artisan, &1);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_cancel_job_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.cancel_job(&finder, &1);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_complete_job_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let artisan = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.complete_job(&artisan, &1);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_confirm_delivery_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.confirm_delivery(&finder, &1);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_auto_release_funds_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let artisan = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.auto_release_funds(&artisan, &1);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_extend_deadline_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.extend_deadline(&finder, &1, &86400u64);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_increase_budget_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);

    market_client.toggle_contract_pause(&admin);
    market_client.increase_budget(&finder, &1, &100);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_transfer_admin_blocked_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.toggle_contract_pause(&admin);
    market_client.transfer_admin(&admin, &new_admin);
}

// ── upgrade tests ─────────────────────────────────────────────────────

#[test]
fn test_upgrade_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    // In the test environment, contracts are stored with empty-bytes WASM.
    // Uploading empty bytes yields a hash that is already present in the ledger.
    let new_wasm_hash = env
        .deployer()
        .upload_contract_wasm(soroban_sdk::Bytes::new(&env));

    market_client.upgrade(&admin, &new_wasm_hash);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_upgrade_wrong_caller() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let new_wasm_hash = BytesN::from_array(&env, &[0u8; 32]);

    market_client.upgrade(&impostor, &new_wasm_hash);
}

#[test]
#[should_panic(expected = "Admin not set")]
fn test_upgrade_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_wasm_hash = BytesN::from_array(&env, &[0u8; 32]);

    client.upgrade(&admin, &new_wasm_hash);
}
