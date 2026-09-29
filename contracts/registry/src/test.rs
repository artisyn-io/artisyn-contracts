use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Env, String, Symbol, TryFromVal,
};

fn setup_env() -> (Env, Address, RegistryClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);
    (env, contract_id, client)
}

fn seed_profile(env: &Env, contract_id: &Address, user: &Address, role: u32) {
    env.as_contract(contract_id, || {
        write_profile(
            env,
            user,
            &Profile {
                role,
                metadata_hash: String::from_str(env, "hash"),
                is_verified: false,
                is_blacklisted: false,
            },
        );
    });
}

fn assert_last_event(env: &Env, contract_id: &Address, name: &str, user: &Address) {
    let events = env.events().all();
    let last_event = events.last().expect("No events were emitted!");

    assert_eq!(last_event.0, *contract_id);

    let topics = last_event.1;

    let event_name: Symbol = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
    assert_eq!(event_name, Symbol::new(env, name));

    let event_user: Address = Address::try_from_val(env, &topics.get(1).unwrap()).unwrap();
    assert_eq!(event_user, *user);
}

fn read_application_status(
    env: &Env,
    contract_id: &Address,
    user: &Address,
) -> Option<VerificationStatus> {
    env.as_contract(contract_id, || read_verification_status(env, user))
}

#[test]
fn test_register_user_success() {
    let (env, contract_id, client) = setup_env();
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.register_user(&user, &String::from_str(&env, "ipfs_cid_123"));

    let events = env.events().all();

    assert!(!events.is_empty(), "No events were emitted!");

    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);

    let topics = last_event.1;

    let event_name: Symbol = Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
    assert_eq!(event_name, Symbol::new(&env, "user_registered"));

    let event_user: Address = Address::try_from_val(&env, &topics.get(1).unwrap()).unwrap();
    assert_eq!(event_user, user);

    let profile = client.get_profile(&user);
    assert_eq!(profile.role, ROLE_FINDER);
    assert_eq!(
        profile.metadata_hash,
        String::from_str(&env, "ipfs_cid_123")
    );
    assert!(!profile.is_verified);
}

#[test]
#[should_panic(expected = "User already registered")]
fn test_register_user_twice_fails() {
    let (env, _contract_id, client) = setup_env();
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.register_user(&user, &String::from_str(&env, "hash1"));
    client.register_user(&user, &String::from_str(&env, "hash2"));
}

#[test]
fn test_remove_curator_demotes_curator_to_finder() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);

    client.remove_curator(&curator);

    let profile_after = client.get_profile(&curator);
    assert_eq!(profile_after.role, ROLE_FINDER);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_remove_curator_panics_for_unregistered_user() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let ghost = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.remove_curator(&ghost);
}

#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_remove_curator_panics_if_not_curator() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    client.remove_curator(&finder);
}

#[test]
fn test_remove_curator_does_not_affect_other_users() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator1 = Address::generate(&env);
    let curator2 = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator1, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &curator2, ROLE_CURATOR);

    client.remove_curator(&curator1);

    assert_eq!(client.get_profile(&curator1).role, ROLE_FINDER);
    assert_eq!(client.get_profile(&curator2).role, ROLE_CURATOR);
}

#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_remove_curator_cannot_be_called_twice() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    client.remove_curator(&curator);
    client.remove_curator(&curator);
}

#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_remove_curator_cannot_demote_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &admin, ROLE_ADMIN);
    client.remove_curator(&admin);
}

#[test]
fn test_approve_artisan_by_curator() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);
    client.apply_for_verification(&finder);

    client.approve_artisan(&curator, &finder);

    let profile_after = client.get_profile(&finder);
    assert_eq!(profile_after.role, ROLE_ARTISAN);
    assert!(profile_after.is_verified);
    assert_eq!(
        read_application_status(&env, &contract_id, &finder),
        Some(VerificationStatus::Approved)
    );
}

#[test]
fn test_approve_artisan_by_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &admin, ROLE_ADMIN);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);
    client.apply_for_verification(&finder);

    client.approve_artisan(&admin, &finder);

    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);
    assert_eq!(
        read_application_status(&env, &contract_id, &finder),
        Some(VerificationStatus::Approved)
    );
}

#[test]
#[should_panic(expected = "Verification application is not pending")]
fn test_approve_artisan_requires_pending_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    client.approve_artisan(&curator, &finder);
}

#[test]
#[should_panic(expected = "Caller must be Curator or Admin")]
fn test_approve_artisan_panics_when_called_by_finder() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder1 = Address::generate(&env);
    let finder2 = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &finder1, ROLE_FINDER);
    seed_profile(&env, &contract_id, &finder2, ROLE_FINDER);

    client.approve_artisan(&finder1, &finder2);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_approve_artisan_panics_for_unregistered_user() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let ghost = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);

    client.approve_artisan(&curator, &ghost);
}

#[test]
fn test_approve_artisan_does_not_affect_other_users() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder1 = Address::generate(&env);
    let finder2 = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder1, ROLE_FINDER);
    seed_profile(&env, &contract_id, &finder2, ROLE_FINDER);
    client.apply_for_verification(&finder1);

    client.approve_artisan(&curator, &finder1);

    assert_eq!(client.get_profile(&finder1).role, ROLE_ARTISAN);
    assert_eq!(client.get_profile(&finder2).role, ROLE_FINDER);
    assert_eq!(client.get_profile(&curator).role, ROLE_CURATOR);
    assert_eq!(
        read_application_status(&env, &contract_id, &finder1),
        Some(VerificationStatus::Approved)
    );
    assert_eq!(read_application_status(&env, &contract_id, &finder2), None);
}

#[test]
#[should_panic(expected = "Verification application is not pending")]
fn test_approve_artisan_cannot_be_called_twice_after_approval() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);
    client.apply_for_verification(&finder);

    client.approve_artisan(&curator, &finder);
    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);

    client.approve_artisan(&curator, &finder);
}

#[test]
fn test_add_curator_by_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    client.add_curator(&finder);

    let profile_after = client.get_profile(&finder);
    assert_eq!(profile_after.role, ROLE_CURATOR);
}

#[test]
fn test_blacklisted_user_state_persisted() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    env.as_contract(&contract_id, || {
        write_profile(
            &env,
            &user,
            &Profile {
                role: ROLE_ARTISAN,
                metadata_hash: String::from_str(&env, "hash"),
                is_verified: true,
                is_blacklisted: true,
            },
        );
    });

    let profile = client.get_profile(&user);
    assert!(profile.is_blacklisted);
    assert_eq!(profile.role, ROLE_ARTISAN);
    assert!(profile.is_verified);
}

#[test]
fn test_full_lifecycle() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator_user = Address::generate(&env);
    let artisan_user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    client.register_user(&curator_user, &String::from_str(&env, "curator_metadata"));
    client.register_user(&artisan_user, &String::from_str(&env, "artisan_metadata"));

    let curator_profile = client.get_profile(&curator_user);
    assert_eq!(curator_profile.role, ROLE_FINDER);

    client.add_curator(&curator_user);
    let curator_profile_after = client.get_profile(&curator_user);
    assert_eq!(curator_profile_after.role, ROLE_CURATOR);

    client.apply_for_verification(&artisan_user);
    client.approve_artisan(&curator_user, &artisan_user);
    let artisan_profile = client.get_profile(&artisan_user);
    assert_eq!(artisan_profile.role, ROLE_ARTISAN);
    assert!(artisan_profile.is_verified);
    assert_eq!(
        read_application_status(&env, &_contract_id, &artisan_user),
        Some(VerificationStatus::Approved)
    );

    client.update_profile_metadata(&artisan_user, &String::from_str(&env, "updated_metadata"));
    let artisan_profile_updated = client.get_profile(&artisan_user);
    assert_eq!(
        artisan_profile_updated.metadata_hash,
        String::from_str(&env, "updated_metadata")
    );
}

#[test]
#[should_panic(expected = "Caller must be Curator or Admin")]
fn test_full_lifecycle_finder_cannot_approve() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let artisan_candidate = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);
    seed_profile(&env, &contract_id, &artisan_candidate, ROLE_FINDER);

    client.approve_artisan(&finder, &artisan_candidate);
}

// ── transfer_admin tests ─────────────────────────────────────────────────────

#[test]
fn test_transfer_admin_success() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&admin, &new_admin);

    // Verify new admin is now in control by transferring again
    let another_admin = Address::generate(&env);
    client.transfer_admin(&new_admin, &another_admin);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_transfer_admin_wrong_caller() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&impostor, &new_admin);
}

#[test]
#[should_panic(expected = "No current admin")]
fn test_transfer_admin_not_initialized() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();

    // No initialize() call — should panic
    client.transfer_admin(&admin, &new_admin);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_transfer_admin_old_admin_cannot_transfer_again() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&admin, &new_admin);

    // old admin tries to reclaim — must fail
    client.transfer_admin(&admin, &admin);
}

#[test]
fn test_transfer_admin_get_admin_reflects_new_admin() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    assert_eq!(client.get_admin(), admin);

    client.transfer_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_transfer_admin_emits_event() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&admin, &new_admin);

    let events = env.events().all();
    let registry_event_count = events.iter().filter(|e| e.0 == contract_id).count();
    assert!(
        registry_event_count >= 1,
        "Expected AdminTransferred event to be emitted"
    );
}

// ── blacklist / unblacklist tests ────────────────────────────────────────────

#[test]
fn test_blacklist_user_by_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    client.blacklist_user(&admin, &user);
    assert_last_event(&env, &contract_id, "user_blacklisted", &user);

    assert!(client.get_profile(&user).is_blacklisted);
}

#[test]
fn test_unblacklist_user_by_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);
    client.blacklist_user(&admin, &user);

    client.unblacklist_user(&admin, &user);
    assert_last_event(&env, &contract_id, "user_unblacklisted", &user);

    assert!(!client.get_profile(&user).is_blacklisted);
}

#[test]
fn test_blacklist_state_transitions_are_repeatable() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    client.blacklist_user(&admin, &user);
    client.unblacklist_user(&admin, &user);
    client.blacklist_user(&admin, &user);

    assert!(client.get_profile(&user).is_blacklisted);
}

#[test]
fn test_blacklist_user_preserves_role_and_verification() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let artisan = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    env.as_contract(&contract_id, || {
        write_profile(
            &env,
            &artisan,
            &Profile {
                role: ROLE_ARTISAN,
                metadata_hash: String::from_str(&env, "artisan_metadata"),
                is_verified: true,
                is_blacklisted: false,
            },
        );
    });

    client.blacklist_user(&admin, &artisan);

    let profile = client.get_profile(&artisan);
    assert!(profile.is_blacklisted);
    assert_eq!(profile.role, ROLE_ARTISAN);
    assert!(profile.is_verified);
    assert_eq!(
        profile.metadata_hash,
        String::from_str(&env, "artisan_metadata")
    );
}

#[test]
fn test_blacklist_user_does_not_affect_other_users() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user1, ROLE_FINDER);
    seed_profile(&env, &contract_id, &user2, ROLE_FINDER);

    client.blacklist_user(&admin, &user1);

    assert!(client.get_profile(&user1).is_blacklisted);
    assert!(!client.get_profile(&user2).is_blacklisted);
}

#[test]
fn test_blacklist_user_by_new_admin_after_transfer() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);
    client.transfer_admin(&admin, &new_admin);

    client.blacklist_user(&new_admin, &user);

    assert!(client.get_profile(&user).is_blacklisted);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_blacklist_user_rejects_non_admin_caller() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    client.blacklist_user(&curator, &user);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_unblacklist_user_rejects_non_admin_caller() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);
    client.blacklist_user(&admin, &user);

    client.unblacklist_user(&curator, &user);
}

#[test]
#[should_panic(expected = "Unauthorized caller")]
fn test_blacklist_user_rejects_old_admin_after_transfer() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);
    client.transfer_admin(&admin, &new_admin);

    client.blacklist_user(&admin, &user);
}

#[test]
#[should_panic]
fn test_blacklist_user_requires_admin_signature() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    env.mock_auths(&[]);
    client.blacklist_user(&admin, &user);
}

#[test]
#[should_panic]
fn test_unblacklist_user_requires_admin_signature() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);
    client.blacklist_user(&admin, &user);

    env.mock_auths(&[]);
    client.unblacklist_user(&admin, &user);
}

#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_blacklist_user_requires_initialization() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    client.blacklist_user(&admin, &user);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_blacklist_user_panics_for_unregistered_user() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let ghost = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    client.blacklist_user(&admin, &ghost);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_unblacklist_user_panics_for_unregistered_user() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let ghost = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    client.unblacklist_user(&admin, &ghost);
}

#[test]
#[should_panic(expected = "User is already blacklisted")]
fn test_blacklist_user_twice_fails() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    client.blacklist_user(&admin, &user);
    client.blacklist_user(&admin, &user);
}

#[test]
#[should_panic(expected = "User is not blacklisted")]
fn test_unblacklist_user_when_not_blacklisted_fails() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &user, ROLE_FINDER);

    client.unblacklist_user(&admin, &user);
}

// ── verification application tests ───────────────────────────────────────────

#[test]
fn test_apply_for_verification_persists_pending_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.apply_for_verification(&applicant);
    assert_last_event(&env, &contract_id, "application_received", &applicant);

    assert_eq!(
        read_application_status(&env, &contract_id, &applicant),
        Some(VerificationStatus::Pending)
    );
    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Pending
    );
    assert!(client.has_verification_application(&applicant));
}

#[test]
fn test_has_verification_application_is_false_before_submission() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    assert!(!client.has_verification_application(&applicant));
}

#[test]
#[should_panic(expected = "Verification application not found")]
fn test_get_verification_status_panics_without_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.get_verification_status(&applicant);
}

#[test]
#[should_panic(expected = "Verification application already pending")]
fn test_apply_for_verification_rejects_duplicate_pending_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.apply_for_verification(&applicant);
    client.apply_for_verification(&applicant);
}

#[test]
#[should_panic(expected = "User is already verified")]
fn test_apply_for_verification_rejects_already_approved_applicant() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.apply_for_verification(&applicant);
    client.approve_artisan(&curator, &applicant);

    client.apply_for_verification(&applicant);
}

#[test]
fn test_apply_for_verification_allowed_again_after_rejection() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.apply_for_verification(&applicant);
    client.reject_artisan(&curator, &applicant);
    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Rejected
    );

    client.apply_for_verification(&applicant);

    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Pending
    );
}

#[test]
fn test_verification_status_full_transition_trail() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.apply_for_verification(&applicant);
    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Pending
    );

    client.reject_artisan(&curator, &applicant);
    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Rejected
    );

    client.apply_for_verification(&applicant);
    client.approve_artisan(&curator, &applicant);

    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Approved
    );
    assert_eq!(client.get_profile(&applicant).role, ROLE_ARTISAN);
    assert!(client.get_profile(&applicant).is_verified);
}

#[test]
#[should_panic(expected = "User not registered")]
fn test_apply_for_verification_panics_for_unregistered_user() {
    let (env, _contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let ghost = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    client.apply_for_verification(&ghost);
}

#[test]
#[should_panic]
fn test_apply_for_verification_requires_applicant_signature() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    env.mock_auths(&[]);
    client.apply_for_verification(&applicant);
}

// ── reject_artisan tests ─────────────────────────────────────────────────────

#[test]
fn test_reject_artisan_by_curator() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    client.reject_artisan(&curator, &applicant);
    assert_last_event(&env, &contract_id, "application_rejected", &applicant);

    assert_eq!(
        read_application_status(&env, &contract_id, &applicant),
        Some(VerificationStatus::Rejected)
    );
}

#[test]
fn test_reject_artisan_by_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &admin, ROLE_ADMIN);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    client.reject_artisan(&admin, &applicant);

    assert_eq!(
        client.get_verification_status(&applicant),
        VerificationStatus::Rejected
    );
}

#[test]
fn test_reject_artisan_leaves_profile_unverified() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    client.reject_artisan(&curator, &applicant);

    let profile = client.get_profile(&applicant);
    assert_eq!(profile.role, ROLE_FINDER);
    assert!(!profile.is_verified);
}

#[test]
fn test_reject_artisan_does_not_affect_other_applicants() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant1 = Address::generate(&env);
    let applicant2 = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant1, ROLE_FINDER);
    seed_profile(&env, &contract_id, &applicant2, ROLE_FINDER);
    client.apply_for_verification(&applicant1);
    client.apply_for_verification(&applicant2);

    client.reject_artisan(&curator, &applicant1);

    assert_eq!(
        client.get_verification_status(&applicant1),
        VerificationStatus::Rejected
    );
    assert_eq!(
        client.get_verification_status(&applicant2),
        VerificationStatus::Pending
    );
}

#[test]
#[should_panic(expected = "Caller must be Curator or Admin")]
fn test_reject_artisan_panics_when_called_by_finder() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    client.reject_artisan(&finder, &applicant);
}

#[test]
#[should_panic(expected = "Caller not registered")]
fn test_reject_artisan_panics_for_unregistered_caller() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let ghost = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    client.reject_artisan(&ghost, &applicant);
}

#[test]
#[should_panic(expected = "User not found")]
fn test_reject_artisan_panics_for_unregistered_applicant() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let ghost = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);

    client.reject_artisan(&curator, &ghost);
}

#[test]
#[should_panic(expected = "Verification application is not pending")]
fn test_reject_artisan_requires_an_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);

    client.reject_artisan(&curator, &applicant);
}

#[test]
#[should_panic(expected = "Verification application is not pending")]
fn test_reject_artisan_cannot_be_called_twice() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    client.reject_artisan(&curator, &applicant);
    client.reject_artisan(&curator, &applicant);
}

#[test]
#[should_panic(expected = "Verification application is not pending")]
fn test_reject_artisan_cannot_reject_approved_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);
    client.approve_artisan(&curator, &applicant);

    client.reject_artisan(&curator, &applicant);
}

#[test]
#[should_panic(expected = "Verification application is not pending")]
fn test_approve_artisan_cannot_approve_rejected_application() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);
    client.reject_artisan(&curator, &applicant);

    client.approve_artisan(&curator, &applicant);
}

#[test]
#[should_panic]
fn test_reject_artisan_requires_caller_signature() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let applicant = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &applicant, ROLE_FINDER);
    client.apply_for_verification(&applicant);

    env.mock_auths(&[]);
    client.reject_artisan(&curator, &applicant);
}
