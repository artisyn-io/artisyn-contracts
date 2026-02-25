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

    client.approve_artisan(&curator, &finder);

    let profile_after = client.get_profile(&finder);
    assert_eq!(profile_after.role, ROLE_ARTISAN);
    assert!(profile_after.is_verified);
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

    client.approve_artisan(&admin, &finder);

    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);
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

    client.approve_artisan(&curator, &finder1);

    assert_eq!(client.get_profile(&finder1).role, ROLE_ARTISAN);
    assert_eq!(client.get_profile(&finder2).role, ROLE_FINDER);
    assert_eq!(client.get_profile(&curator).role, ROLE_CURATOR);
}

#[test]
fn test_approve_artisan_is_idempotent() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    client.approve_artisan(&curator, &finder);
    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);

    client.approve_artisan(&curator, &finder);
    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);
}
