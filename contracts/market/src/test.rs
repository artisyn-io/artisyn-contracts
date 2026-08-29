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

    let job_id = client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    market_client.assign_artisan(&finder, &job_id, &artisan);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
}

#[test]
#[should_panic(expected = "Assignment has not timed out")]
fn test_reopen_assignment_just_before_timeout() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let assigned_at = 1_000;
    env.ledger().with_mut(|li| li.timestamp = assigned_at);
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    env.ledger()
        .with_mut(|li| li.timestamp = assigned_at + ASSIGNMENT_TIMEOUT_SECONDS - 1);
    market_client.reopen_timed_out_assignment(&finder, &job_id);
}

#[test]
fn test_reopen_assignment_after_timeout_preserves_escrow_and_allows_reassignment() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let first_artisan = Address::generate(&env);
    let second_artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &first_artisan, 3);
    seed_artisan_profile(&env, &registry_id, &second_artisan, 3);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let assigned_at = 1_000;
    env.ledger().with_mut(|li| li.timestamp = assigned_at);
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &first_artisan);

    env.ledger()
        .with_mut(|li| li.timestamp = assigned_at + ASSIGNMENT_TIMEOUT_SECONDS);
    market_client.reopen_timed_out_assignment(&finder, &job_id);

    let reopened_job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap()
    });
    assert_eq!(reopened_job.status, JobStatus::Open);
    assert_eq!(reopened_job.artisan, None);
    assert_eq!(reopened_job.amount, 500);
    assert_eq!(token_client.balance(&market_id), 500);
    assert_eq!(token_client.balance(&finder), 500);

    market_client.assign_artisan(&finder, &job_id, &second_artisan);
    let reassigned_job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap()
    });
    assert_eq!(reassigned_job.status, JobStatus::Assigned);
    assert_eq!(reassigned_job.artisan, Some(second_artisan));
    assert_eq!(token_client.balance(&market_id), 500);
}

#[test]
fn test_reassign_artisan_after_timeout_preserves_escrow_and_applications() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let previous_artisan = Address::generate(&env);
    let new_artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &previous_artisan, 3);
    seed_artisan_profile(&env, &registry_id, &new_artisan, 3);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let assigned_at = 1_000;
    env.ledger().with_mut(|li| li.timestamp = assigned_at);
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.apply_for_job(&new_artisan, &job_id);
    market_client.assign_artisan(&finder, &job_id, &previous_artisan);

    env.ledger()
        .with_mut(|li| li.timestamp = assigned_at + ASSIGNMENT_TIMEOUT_SECONDS);
    market_client.reassign_artisan(&finder, &job_id, &new_artisan);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap()
    });
    assert_eq!(job.status, JobStatus::Assigned);
    assert_eq!(job.artisan, Some(new_artisan.clone()));
    assert_eq!(token_client.balance(&market_id), 500);
    assert!(market_client.has_applied(&job_id, &new_artisan));
}

#[test]
#[should_panic(expected = "Assignment has not timed out")]
fn test_reassign_artisan_before_timeout_is_blocked() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let previous_artisan = Address::generate(&env);
    let new_artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &previous_artisan, 3);
    seed_artisan_profile(&env, &registry_id, &new_artisan, 3);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &previous_artisan);

    env.ledger()
        .with_mut(|li| li.timestamp = ASSIGNMENT_TIMEOUT_SECONDS - 1);
    market_client.reassign_artisan(&finder, &job_id, &new_artisan);
}

#[test]
#[should_panic(expected = "Job is not assigned")]
fn test_reassign_artisan_after_job_started_is_blocked() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let previous_artisan = Address::generate(&env);
    let new_artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &previous_artisan, 3);
    seed_artisan_profile(&env, &registry_id, &new_artisan, 3);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &previous_artisan);
    market_client.start_job(&previous_artisan, &job_id);

    market_client.reassign_artisan(&finder, &job_id, &new_artisan);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    assert!(!market_client.has_applied(&job_id, &artisan));
    assert_eq!(market_client.get_job_applicants(&job_id).len(), 0);
    assert_eq!(market_client.get_application(&job_id, &artisan), None);

    market_client.apply_for_job(&artisan, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);

    assert!(market_client.has_applied(&job_id, &artisan));

    let applicants = market_client.get_job_applicants(&job_id);
    assert_eq!(applicants.len(), 1);
    assert_eq!(applicants.get(0).unwrap(), artisan);

    let app_record = market_client.get_application(&job_id, &artisan).unwrap();
    assert_eq!(app_record.job_id, job_id);
    assert_eq!(app_record.artisan, artisan);
}

#[test]
#[should_panic(expected = "Duplicate application")]
fn test_apply_for_job_duplicate_rejected() {
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    market_client.apply_for_job(&artisan, &job_id);
    market_client.apply_for_job(&artisan, &job_id);
}

#[test]
fn test_apply_for_job_multiple_applicants_persistence() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan1 = Address::generate(&env);
    let artisan2 = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(&env, &registry_id, &artisan1, 3);
    seed_artisan_profile(&env, &registry_id, &artisan2, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    market_client.apply_for_job(&artisan1, &job_id);
    market_client.apply_for_job(&artisan2, &job_id);

    let applicants = market_client.get_job_applicants(&job_id);
    assert_eq!(applicants.len(), 2);
    assert_eq!(applicants.get(0).unwrap(), artisan1);
    assert_eq!(applicants.get(1).unwrap(), artisan2);

    assert!(market_client.has_applied(&job_id, &artisan1));
    assert!(market_client.has_applied(&job_id, &artisan2));
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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
#[should_panic(expected = "User is blacklisted")]
fn test_apply_for_job_blocked_after_registry_blacklist() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    registry_client.blacklist_user(&admin, &artisan);

    market_client.apply_for_job(&artisan, &job_id);
}

#[test]
fn test_apply_for_job_allowed_after_registry_unblacklist() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    registry_client.blacklist_user(&admin, &artisan);
    registry_client.unblacklist_user(&admin, &artisan);

    market_client.apply_for_job(&artisan, &job_id);

    let events = env.events().all();
    let market_event_count = events.iter().filter(|e| e.0 == market_id).count();
    assert!(market_event_count >= 1);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let reason = String::from_str(&env, "Work quality does not meet requirements");
    market_client.raise_dispute(&finder, &job_id, &reason);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    let reason = String::from_str(&env, "Payment not received as agreed");
    market_client.raise_dispute(&artisan, &job_id, &reason);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let reason = String::from_str(&env, "Random dispute");
    market_client.raise_dispute(&random_user, &job_id, &reason);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    let reason = String::from_str(&env, "Wrong status test");
    market_client.raise_dispute(&finder, &job_id, &reason);
}

#[test]
fn test_raise_dispute_stores_reason_from_finder() {
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let expected_reason = String::from_str(&env, "Deliverable does not match specifications");
    market_client.raise_dispute(&finder, &job_id, &expected_reason);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });

    assert_eq!(job.status, JobStatus::Disputed);
    assert!(job.dispute_reason.is_some());
    assert_eq!(job.dispute_reason.unwrap(), expected_reason);
}

#[test]
fn test_raise_dispute_stores_reason_from_artisan() {
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    let expected_reason = String::from_str(&env, "Finder refusing to pay agreed amount");
    market_client.raise_dispute(&artisan, &job_id, &expected_reason);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });

    assert_eq!(job.status, JobStatus::Disputed);
    assert!(job.dispute_reason.is_some());
    assert_eq!(job.dispute_reason.unwrap(), expected_reason);
}

#[test]
fn test_raise_dispute_event_contains_reason() {
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let expected_reason = String::from_str(&env, "Scope creep without compensation");
    market_client.raise_dispute(&finder, &job_id, &expected_reason);

    let events = env.events().all();
    let dispute_event_count = events.iter().filter(|e| e.0 == market_id).count();

    assert!(dispute_event_count > 0);
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
            deadline: 30 * 24 * 60 * 60,
            total_extended: 0,
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

    // 1% of 500 = 5. So artisan gets 495, admin gets 5.
    assert_eq!(token_client.balance(&artisan), 495);
    assert_eq!(token_client.balance(&admin), 5);
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
            deadline: 30 * 24 * 60 * 60,
            total_extended: 0,
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &300, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
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
    market_client.create_job(&finder, &token_client.address, &500, &0);
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

// ── emergency_withdraw tests ─────────────────────────────────────────────────

#[test]
fn test_emergency_withdraw_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _, _) = setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let rescue_target = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    market_client.create_job(&finder, &token_client.address, &500, &0);

    assert_eq!(token_client.balance(&market_id), 500);
    assert_eq!(token_client.balance(&rescue_target), 0);

    market_client.toggle_contract_pause(&admin);
    market_client.emergency_withdraw(&admin, &token_client.address, &500, &rescue_target);

    assert_eq!(token_client.balance(&market_id), 0);
    assert_eq!(token_client.balance(&rescue_target), 500);
}

#[test]
fn test_emergency_withdraw_partial_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _, _) = setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let rescue_target = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    market_client.create_job(&finder, &token_client.address, &500, &0);

    market_client.toggle_contract_pause(&admin);
    market_client.emergency_withdraw(&admin, &token_client.address, &200, &rescue_target);

    assert_eq!(token_client.balance(&market_id), 300);
    assert_eq!(token_client.balance(&rescue_target), 200);
}

#[test]
#[should_panic(expected = "Contract is not paused")]
fn test_emergency_withdraw_fails_when_not_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_, market_client, _, _) = setup_market_and_registry(&env, admin.clone());

    let rescue_target = Address::generate(&env);
    let (token_client, _) = create_token(&env, &admin);

    market_client.emergency_withdraw(&admin, &token_client.address, &100, &rescue_target);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_emergency_withdraw_fails_for_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (_, market_client, _, _) = setup_market_and_registry(&env, admin.clone());

    let rescue_target = Address::generate(&env);
    let (token_client, _) = create_token(&env, &admin);

    market_client.toggle_contract_pause(&admin);
    market_client.emergency_withdraw(&impostor, &token_client.address, &100, &rescue_target);
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

// ── set_platform_fee tests ───────────────────────────────────────────────────

#[test]
fn test_set_platform_fee_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.set_platform_fee(&admin, &100);
}

#[test]
fn test_set_platform_fee_at_hardcap() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    // Exactly 1000 bps (10%) should be allowed
    market_client.set_platform_fee(&admin, &1000);
}

#[test]
#[should_panic(expected = "Fee exceeds maximum allowed (1000 bps)")]
fn test_set_platform_fee_exceeds_hardcap() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.set_platform_fee(&admin, &1001);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_set_platform_fee_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.set_platform_fee(&impostor, &100);
}

#[test]
#[should_panic(expected = "Admin not set")]
fn test_set_platform_fee_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_platform_fee(&admin, &100);
}

#[test]
fn test_set_platform_fee_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    market_client.set_platform_fee(&admin, &250);

    let events = env.events().all();
    assert!(!events.is_empty());
}

// ── assign_juror tests ───────────────────────────────────────────────────────

fn create_disputed_job(
    env: &Env,
    market_client: &MarketContractClient,
    registry_id: &Address,
    registry_client: &::registry::RegistryClient,
    admin: &Address,
) -> (u64, Address, Address) {
    let finder = Address::generate(env);
    let artisan = Address::generate(env);

    registry_client.initialize(admin);

    let (token_client, token_admin_client) = create_token(env, admin);
    token_admin_client.mint(&finder, &1000);

    seed_artisan_profile(env, registry_id, &artisan, 3);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let reason = String::from_str(env, "Disputed job for testing");
    market_client.raise_dispute(&finder, &job_id, &reason);

    (job_id, finder, artisan)
}

#[test]
fn test_assign_juror_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let (job_id, _finder, _artisan) =
        create_disputed_job(&env, &market_client, &registry_id, &registry_client, &admin);

    let juror = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &juror, 1); // Curator role

    market_client.assign_juror(&admin, &job_id, &juror);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.juror, Some(juror));
}

#[test]
fn test_assign_juror_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let (job_id, _finder, _artisan) =
        create_disputed_job(&env, &market_client, &registry_id, &registry_client, &admin);

    let juror = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &juror, 1);

    market_client.assign_juror(&admin, &job_id, &juror);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_assign_juror_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let (job_id, _finder, _artisan) =
        create_disputed_job(&env, &market_client, &registry_id, &registry_client, &admin);

    let juror = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &juror, 1);

    market_client.assign_juror(&impostor, &job_id, &juror);
}

#[test]
#[should_panic(expected = "Job is not disputed")]
fn test_assign_juror_job_not_disputed() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    registry_client.initialize(&admin);

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    // Job is InProgress, not Disputed
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let juror = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &juror, 1);

    market_client.assign_juror(&admin, &job_id, &juror);
}

#[test]
#[should_panic(expected = "Job not found")]
fn test_assign_juror_job_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let juror = Address::generate(&env);
    market_client.assign_juror(&admin, &999, &juror);
}

#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_assign_juror_not_curator() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let (job_id, _finder, _artisan) =
        create_disputed_job(&env, &market_client, &registry_id, &registry_client, &admin);

    let juror = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &juror, 3); // Artisan role, not Curator

    market_client.assign_juror(&admin, &job_id, &juror);
}

#[test]
fn test_fee_math_500_bps() {
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

    // Set Platform Fee to 500 BPS (5%)
    market_client.set_platform_fee(&admin, &500);

    let job_id = market_client.create_job(&finder, &token_client.address, &1000, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    market_client.confirm_delivery(&finder, &job_id);

    // 5% fee on 1000 => 50 to admin, 950 to artisan
    assert_eq!(token_client.balance(&artisan), 950);
    assert_eq!(token_client.balance(&admin), 50);
    assert_eq!(token_client.balance(&market_id), 0);
}

// ── circuit breaker integration tests ───────────────────────────────────────

#[test]
fn test_circuit_breaker_full_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let rescue_target = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &2000);

    // Step 1: Normal operations work before pause
    let _job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    assert_eq!(token_client.balance(&market_id), 500);

    // Step 2: Pause the contract
    market_client.toggle_contract_pause(&admin);

    // Step 3: Emergency withdraw succeeds while paused
    market_client.emergency_withdraw(&admin, &token_client.address, &500, &rescue_target);
    assert_eq!(token_client.balance(&market_id), 0);
    assert_eq!(token_client.balance(&rescue_target), 500);

    // Step 4: Unpause the contract
    market_client.toggle_contract_pause(&admin);

    // Step 5: Verify normal operations work again after unpause
    let job_id_2 = market_client.create_job(&finder, &token_client.address, &400, &0);
    assert_eq!(token_client.balance(&market_id), 400);
    market_client.assign_artisan(&finder, &job_id_2, &artisan);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_circuit_breaker_create_job_blocked_during_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    market_client.toggle_contract_pause(&admin);
    market_client.create_job(&finder, &token_client.address, &500, &0);
}

#[test]
#[should_panic(expected = "Contract Paused")]
fn test_circuit_breaker_confirm_delivery_blocked_during_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    market_client.toggle_contract_pause(&admin);
    market_client.confirm_delivery(&finder, &job_id);
}

#[test]
fn test_circuit_breaker_emergency_withdraw_succeeds_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let rescue_target = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    market_client.create_job(&finder, &token_client.address, &500, &0);
    assert_eq!(token_client.balance(&market_id), 500);

    market_client.toggle_contract_pause(&admin);
    market_client.emergency_withdraw(&admin, &token_client.address, &500, &rescue_target);

    assert_eq!(token_client.balance(&market_id), 0);
    assert_eq!(token_client.balance(&rescue_target), 500);
}

#[test]
fn test_circuit_breaker_unpause_restores_operations() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &2000);

    // Pause and unpause
    market_client.toggle_contract_pause(&admin);
    market_client.toggle_contract_pause(&admin);

    // All operations should work after unpause
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    assert_eq!(token_client.balance(&market_id), 500);

    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);
    market_client.confirm_delivery(&finder, &job_id);
}

#[test]
fn test_circuit_breaker_admin_functions_work_during_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let rescue_target = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    market_client.create_job(&finder, &token_client.address, &500, &0);

    // Pause the contract
    market_client.toggle_contract_pause(&admin);

    // Emergency withdraw works during pause
    market_client.emergency_withdraw(&admin, &token_client.address, &500, &rescue_target);
    assert_eq!(token_client.balance(&rescue_target), 500);

    // Toggle pause works (to unpause)
    market_client.toggle_contract_pause(&admin);

    // Verify contract is unpaused by creating a job
    token_admin_client.mint(&finder, &500);
    let _job_id = market_client.create_job(&finder, &token_client.address, &300, &0);
}

// ── auto_release_funds time-travel integration tests ────────────────────────
//
// These tests verify the 7-day auto-release window using Soroban's time-travel features.
// Requirements:
// 1. Finish a job
// 2. Attempt to call auto_release immediately (Must panic) - see test_auto_release_time_travel_immediate_attempt_fails
// 3. Fast forward by 8 days
// 4. Call auto_release (Must succeed) - see test_auto_release_time_travel_exactly_8_days_succeeds
//
// Note: Due to no_std environment, we cannot use catch_unwind to test panic + success in one test.
// The full flow is demonstrated across multiple tests below.

#[test]
fn test_auto_release_time_travel_full_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // REQUIREMENT 1: Finish a job
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    // Set initial timestamp
    let completion_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time;
    });

    market_client.complete_job(&artisan, &job_id);

    // Verify job is in PendingReview status
    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.status, JobStatus::PendingReview);
    assert_eq!(job.end_time, completion_time);

    // REQUIREMENT 2: Attempt to call auto_release immediately would panic here
    // (tested separately in test_auto_release_time_travel_immediate_attempt_fails)

    // REQUIREMENT 3: Fast forward by 8 days
    let eight_days = 8 * 24 * 60 * 60; // 691200 seconds
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time + eight_days;
    });

    assert_eq!(token_client.balance(&artisan), 0);
    assert_eq!(token_client.balance(&market_id), 500);

    // REQUIREMENT 4: Call auto_release (Must succeed)
    market_client.auto_release_funds(&artisan, &job_id);

    // Verify funds were released (1% fee = 5, artisan gets 495)
    assert_eq!(token_client.balance(&artisan), 495);
    assert_eq!(token_client.balance(&admin), 5);
    assert_eq!(token_client.balance(&market_id), 0);
}

#[test]
#[should_panic(expected = "7 days have not passed since job completion")]
fn test_auto_release_time_travel_immediate_attempt_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let completion_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time;
    });

    market_client.complete_job(&artisan, &job_id);

    // Attempt auto_release immediately (0 seconds after completion)
    market_client.auto_release_funds(&artisan, &job_id);
}

#[test]
#[should_panic(expected = "7 days have not passed since job completion")]
fn test_auto_release_time_travel_six_days_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let completion_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time;
    });

    market_client.complete_job(&artisan, &job_id);

    // Fast forward 6 days (still too early)
    let six_days = 6 * 24 * 60 * 60; // 518400 seconds
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time + six_days;
    });

    market_client.auto_release_funds(&artisan, &job_id);
}

#[test]
fn test_auto_release_time_travel_exactly_7_days_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let completion_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time;
    });

    market_client.complete_job(&artisan, &job_id);

    // Fast forward exactly 7 days + 1 second (minimum required)
    let seven_days_plus_one = 7 * 24 * 60 * 60 + 1; // 604801 seconds
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time + seven_days_plus_one;
    });

    assert_eq!(token_client.balance(&artisan), 0);

    market_client.auto_release_funds(&artisan, &job_id);

    assert_eq!(token_client.balance(&artisan), 495);
}

#[test]
fn test_auto_release_time_travel_exactly_8_days_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let completion_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time;
    });

    market_client.complete_job(&artisan, &job_id);

    // Fast forward exactly 8 days (691200 seconds)
    let eight_days = 8 * 24 * 60 * 60; // 691200 seconds
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time + eight_days;
    });

    assert_eq!(token_client.balance(&artisan), 0);

    market_client.auto_release_funds(&artisan, &job_id);

    // Verify funds released with 1% fee
    assert_eq!(token_client.balance(&artisan), 495);
    assert_eq!(token_client.balance(&admin), 5);
}

#[test]
#[should_panic(expected = "7 days have not passed since job completion")]
fn test_auto_release_time_travel_exactly_7_days_minus_one_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    registry_client.initialize(&admin);
    seed_artisan_profile(&env, &registry_id, &artisan, 3);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);

    let completion_time = 1000u64;
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time;
    });

    market_client.complete_job(&artisan, &job_id);

    // Fast forward exactly 7 days (604800 seconds) - should still fail
    let exactly_seven_days = 7 * 24 * 60 * 60; // 604800 seconds
    env.ledger().with_mut(|li| {
        li.timestamp = completion_time + exactly_seven_days;
    });

    market_client.auto_release_funds(&artisan, &job_id);
}

// ── cross-contract E2E tests ─────────────────────────────────────────────────

#[test]
fn test_e2e_cross_contract_full_user_journey() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);

    // Initialize Registry
    registry_client.initialize(&admin);

    // Admin must register themselves to have a profile
    registry_client.register_user(&admin, &String::from_str(&env, "ipfs://admin"));

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Step 1: Create a job in Market
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    // Step 2: Register user in Registry as Finder (role 0)
    registry_client.register_user(&artisan, &String::from_str(&env, "ipfs://metadata"));

    // Verify user is registered but not yet an Artisan
    let profile = registry_client.get_profile(&artisan);
    assert_eq!(profile.role, 0); // ROLE_FINDER
    assert!(!profile.is_verified);

    // Step 3: Admin promotes user to Artisan
    // Note: Admin is registered as ROLE_FINDER (0) but approve_artisan checks for ROLE_ADMIN (2)
    // We need to manually set admin's role to ROLE_ADMIN for this to work
    env.as_contract(&registry_id, || {
        use soroban_sdk::String;
        let admin_profile = ::registry::Profile {
            role: 2, // ROLE_ADMIN
            metadata_hash: String::from_str(&env, "ipfs://admin"),
            is_verified: false,
            is_blacklisted: false,
        };
        env.storage()
            .persistent()
            .set(&::registry::DataKey::Profile(admin.clone()), &admin_profile);
    });

    // Artisan must apply before approval
    registry_client.apply_for_verification(&artisan);
    registry_client.approve_artisan(&admin, &artisan);

    // Verify user is now an Artisan
    let profile = registry_client.get_profile(&artisan);
    assert_eq!(profile.role, 3); // ROLE_ARTISAN
    assert!(profile.is_verified);

    // Step 4: Successfully assign Artisan in Market
    market_client.assign_artisan(&finder, &job_id, &artisan);

    // Verify job was assigned
    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.artisan, Some(artisan.clone()));
    assert_eq!(job.status, JobStatus::Assigned);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_e2e_cross_contract_unregistered_user_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let unregistered_artisan = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    // Attempt to assign unregistered user - should panic with "User not found"
    market_client.assign_artisan(&finder, &job_id, &unregistered_artisan);
}

#[test]
#[should_panic(expected = "User is not a verified Artisan")]
fn test_e2e_cross_contract_finder_cannot_be_assigned() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let registered_finder = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Register user as Finder
    registry_client.register_user(
        &registered_finder,
        &String::from_str(&env, "ipfs://metadata"),
    );

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    // Attempt to assign Finder (role 0) - should panic
    market_client.assign_artisan(&finder, &job_id, &registered_finder);
}

#[test]
fn test_e2e_cross_contract_curator_workflow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let finder = Address::generate(&env);
    let artisan = Address::generate(&env);
    let curator = Address::generate(&env);

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Register curator and artisan
    registry_client.register_user(&curator, &String::from_str(&env, "ipfs://curator"));
    registry_client.register_user(&artisan, &String::from_str(&env, "ipfs://artisan"));

    // Admin promotes curator
    registry_client.add_curator(&curator);

    // Artisan must apply before curator can approve
    registry_client.apply_for_verification(&artisan);
    // Curator approves artisan
    registry_client.approve_artisan(&curator, &artisan);

    // Verify artisan can be assigned in Market
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });
    assert_eq!(job.artisan, Some(artisan));
    assert_eq!(job.status, JobStatus::Assigned);
}

// ── deadline policy tests ────────────────────────────────────────────────────

#[test]
fn test_create_job_with_initial_deadline() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let custom_deadline = 7 * 24 * 60 * 60; // 7 days
    let job_id = market_client.create_job(&finder, &token_client.address, &500, &custom_deadline);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap()
    });
    assert_eq!(job.deadline, custom_deadline);
    assert_eq!(job.total_extended, 0);
}

#[test]
fn test_create_job_with_zero_deadline_uses_default() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap()
    });
    assert_eq!(job.deadline, DEFAULT_DEADLINE_SECONDS);
}

#[test]
fn test_extend_deadline_within_cap_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    let one_day: u64 = 24 * 60 * 60;
    market_client.extend_deadline(&finder, &job_id, &one_day);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .unwrap()
    });
    assert_eq!(job.deadline, DEFAULT_DEADLINE_SECONDS + one_day);
    assert_eq!(job.total_extended, one_day);
}

#[test]
#[should_panic(expected = "Extension must be greater than zero")]
fn test_extend_deadline_zero_extra_time_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    market_client.extend_deadline(&finder, &job_id, &0);
}

#[test]
#[should_panic(expected = "Extension exceeds maximum single extension")]
fn test_extend_deadline_exceeds_single_cap_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    let too_much = MAX_SINGLE_EXTENSION_SECONDS + 1;
    market_client.extend_deadline(&finder, &job_id, &too_much);
}

#[test]
#[should_panic(expected = "Cumulative extension exceeds cap")]
fn test_extend_deadline_cumulative_cap_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Set small caps for easy testing: 7-day default, 3-day max single, 5-day cumulative
    let three_days: u64 = 3 * 24 * 60 * 60;
    let five_days: u64 = 5 * 24 * 60 * 60;
    let seven_days: u64 = 7 * 24 * 60 * 60;
    market_client.set_deadline_policy(&admin, &seven_days, &three_days, &five_days);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    // First extension: 3 days (at single cap) — total = 3 days
    market_client.extend_deadline(&finder, &job_id, &three_days);

    // Second extension: 3 days — total would be 6 days > 5-day cumulative cap
    market_client.extend_deadline(&finder, &job_id, &three_days);
}

#[test]
fn test_extend_deadline_cumulative_cap_at_boundary_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Set small caps: 7-day default, 3-day max single, 6-day cumulative
    let three_days: u64 = 3 * 24 * 60 * 60;
    let six_days: u64 = 6 * 24 * 60 * 60;
    let seven_days: u64 = 7 * 24 * 60 * 60;
    market_client.set_deadline_policy(&admin, &seven_days, &three_days, &six_days);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    // Two extensions of 3 days each = 6 days total (exactly at cumulative cap)
    market_client.extend_deadline(&finder, &job_id, &three_days);
    market_client.extend_deadline(&finder, &job_id, &three_days);
}

#[test]
fn test_set_deadline_policy_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let one_week: u64 = 7 * 24 * 60 * 60;
    let two_weeks: u64 = 14 * 24 * 60 * 60;
    let four_weeks: u64 = 28 * 24 * 60 * 60;

    market_client.set_deadline_policy(&admin, &one_week, &two_weeks, &four_weeks);

    let (default_d, max_single, max_cum) = market_client.get_deadline_policy();
    assert_eq!(default_d, one_week);
    assert_eq!(max_single, two_weeks);
    assert_eq!(max_cum, four_weeks);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_set_deadline_policy_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let one_week: u64 = 7 * 24 * 60 * 60;
    let two_weeks: u64 = 14 * 24 * 60 * 60;
    let four_weeks: u64 = 28 * 24 * 60 * 60;

    market_client.set_deadline_policy(&impostor, &one_week, &two_weeks, &four_weeks);
}

#[test]
#[should_panic(expected = "Max single extension must not exceed cumulative cap")]
fn test_set_deadline_policy_single_exceeds_cumulative_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let one_week: u64 = 7 * 24 * 60 * 60;
    let two_weeks: u64 = 14 * 24 * 60 * 60;

    // max_single (two weeks) > max_cumulative (one week)
    market_client.set_deadline_policy(&admin, &one_week, &two_weeks, &one_week);
}

#[test]
#[should_panic(expected = "Extension exceeds maximum single extension")]
fn test_custom_deadline_policy_enforced_on_extend() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, _registry_id, _registry_client) =
        setup_market_and_registry(&env, admin.clone());
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    // Set custom policy: 7 day default, 3 day max single, 14 day cumulative
    let three_days: u64 = 3 * 24 * 60 * 60;
    let seven_days: u64 = 7 * 24 * 60 * 60;
    let fourteen_days: u64 = 14 * 24 * 60 * 60;
    market_client.set_deadline_policy(&admin, &seven_days, &three_days, &fourteen_days);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);

    // 4 days exceeds max single extension (3 days)
    let four_days: u64 = 4 * 24 * 60 * 60;
    market_client.extend_deadline(&finder, &job_id, &four_days);
}

// ── platform fee accounting tests ───────────────────────────────────────────

#[test]
fn test_confirm_delivery_records_fee_and_emits_event() {
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    assert_eq!(market_client.get_collected_fees(&token_client.address), 0);

    let events_before = env.events().all().len();
    market_client.confirm_delivery(&finder, &job_id);
    let events_after = env.events().all().len();

    // 1% fee on 500 => 5
    assert_eq!(market_client.get_collected_fees(&token_client.address), 5);
    assert!(events_after > events_before);
}

#[test]
fn test_auto_release_funds_records_fee() {
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

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });

    env.ledger().with_mut(|li| {
        li.timestamp = job.end_time + 604800 + 1;
    });

    assert_eq!(market_client.get_collected_fees(&token_client.address), 0);

    market_client.auto_release_funds(&artisan, &job_id);

    // 1% fee on 500 => 5
    assert_eq!(market_client.get_collected_fees(&token_client.address), 5);
}

#[test]
fn test_resolve_dispute_records_fee() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    let (job_id, _finder, _artisan) =
        create_disputed_job(&env, &market_client, &registry_id, &registry_client, &admin);

    let job: Job = env.as_contract(&market_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found")
    });

    let juror = Address::generate(&env);
    seed_artisan_profile(&env, &registry_id, &juror, 1);
    market_client.assign_juror(&admin, &job_id, &juror);

    assert_eq!(market_client.get_collected_fees(&job.token), 0);

    // 1% fee on 500 => 5, remaining 495 split between finder and artisan
    market_client.resolve_dispute(&juror, &job_id, &200, &295);

    assert_eq!(market_client.get_collected_fees(&job.token), 5);
}

#[test]
fn test_fee_accounting_reconciles_across_all_payout_paths() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (_market_id, market_client, registry_id, registry_client) =
        setup_market_and_registry(&env, admin.clone());

    registry_client.initialize(&admin);

    let (token_client, token_admin_client) = create_token(&env, &admin);

    let finder_a = Address::generate(&env);
    let artisan_a = Address::generate(&env);
    let finder_b = Address::generate(&env);
    let artisan_b = Address::generate(&env);
    let finder_c = Address::generate(&env);
    let artisan_c = Address::generate(&env);
    let juror = Address::generate(&env);

    token_admin_client.mint(&finder_a, &500);
    token_admin_client.mint(&finder_b, &300);
    token_admin_client.mint(&finder_c, &400);

    seed_artisan_profile(&env, &registry_id, &artisan_a, 3);
    seed_artisan_profile(&env, &registry_id, &artisan_b, 3);
    seed_artisan_profile(&env, &registry_id, &artisan_c, 3);
    seed_artisan_profile(&env, &registry_id, &juror, 1);

    // Job A: standard completion via confirm_delivery. 1% of 500 => 5.
    let job_a = market_client.create_job(&finder_a, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder_a, &job_a, &artisan_a);
    market_client.start_job(&artisan_a, &job_a);
    market_client.complete_job(&artisan_a, &job_a);
    market_client.confirm_delivery(&finder_a, &job_a);

    // Job B: auto-release after the finder review window lapses. 1% of 300 => 3.
    let job_b = market_client.create_job(&finder_b, &token_client.address, &300, &0);
    market_client.assign_artisan(&finder_b, &job_b, &artisan_b);
    market_client.start_job(&artisan_b, &job_b);
    market_client.complete_job(&artisan_b, &job_b);

    // Job C: disputed and resolved by a juror. 1% of 400 => 4.
    let job_c = market_client.create_job(&finder_c, &token_client.address, &400, &0);
    market_client.assign_artisan(&finder_c, &job_c, &artisan_c);
    market_client.start_job(&artisan_c, &job_c);
    let reason_c = String::from_str(&env, "Quality issue requiring juror review");
    market_client.raise_dispute(&finder_c, &job_c, &reason_c);
    market_client.assign_juror(&admin, &job_c, &juror);
    market_client.resolve_dispute(&juror, &job_c, &200, &196);

    // Advance time to unlock job B's auto-release.
    env.ledger().with_mut(|li| {
        li.timestamp += 604800 + 1;
    });
    market_client.auto_release_funds(&artisan_b, &job_b);

    assert_eq!(market_client.get_collected_fees(&token_client.address), 12);
}

#[test]
fn test_zero_fee_is_not_recorded_or_transferred() {
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

    market_client.set_platform_fee(&admin, &0);

    let job_id = market_client.create_job(&finder, &token_client.address, &500, &0);
    market_client.assign_artisan(&finder, &job_id, &artisan);
    market_client.start_job(&artisan, &job_id);
    market_client.complete_job(&artisan, &job_id);
    market_client.confirm_delivery(&finder, &job_id);

    assert_eq!(token_client.balance(&admin), 0);
    assert_eq!(market_client.get_collected_fees(&token_client.address), 0);
}
