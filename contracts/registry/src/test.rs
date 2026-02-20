#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Events}, vec, Address, Env, IntoVal, Map, Symbol, Val};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, RegistryContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);
    (env, client)
}

fn register(client: &RegistryContractClient, user: &Address, role: Role) {
    client.register_user(user, &role);
}

// ---------------------------------------------------------------------------
// blacklist_user — happy paths
// ---------------------------------------------------------------------------

#[test]
fn test_blacklist_by_curator_sets_flag() {
    let (env, client) = setup();

    let curator = Address::generate(&env);
    let target = Address::generate(&env);

    register(&client, &curator, Role::Curator);
    register(&client, &target, Role::User);

    client.blacklist_user(&curator, &target);

    let profile = client.get_profile(&target);
    assert!(profile.is_blacklisted);
}

#[test]
fn test_blacklist_by_admin_sets_flag() {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let target = Address::generate(&env);

    register(&client, &admin, Role::Admin);
    register(&client, &target, Role::User);

    client.blacklist_user(&admin, &target);

    let profile = client.get_profile(&target);
    assert!(profile.is_blacklisted);
}

// ---------------------------------------------------------------------------
// blacklist_user — event emission
// ---------------------------------------------------------------------------

#[test]
fn test_blacklist_emits_event() {
    let (env, client) = setup();
    let contract_id = client.address.clone();

    let curator = Address::generate(&env);
    let target = Address::generate(&env);

    register(&client, &curator, Role::Curator);
    register(&client, &target, Role::User);

    client.blacklist_user(&curator, &target);

    // Only our contract emits events (no token transfers here)
    let all_events = env.events().all();
    assert_eq!(all_events.len(), 1);

    let expected_topics: soroban_sdk::Vec<Val> =
        vec![&env, Symbol::new(&env, "user_blacklisted").into_val(&env)];
    let expected_data: Val = Map::<Symbol, Val>::from_array(
        &env,
        [(Symbol::new(&env, "target_user"), target.into_val(&env))],
    )
    .into_val(&env);

    let expected: soroban_sdk::Vec<(Address, soroban_sdk::Vec<Val>, Val)> = vec![
        &env,
        (contract_id.clone(), expected_topics, expected_data),
    ];
    assert_eq!(vec![&env, all_events.get(0).unwrap()], expected);
}

// ---------------------------------------------------------------------------
// blacklist_user — error paths
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Target user profile not found")]
fn test_blacklist_panics_if_target_not_registered() {
    let (env, client) = setup();

    let curator = Address::generate(&env);
    let ghost = Address::generate(&env);

    register(&client, &curator, Role::Curator);

    client.blacklist_user(&curator, &ghost);
}

#[test]
#[should_panic(expected = "Unauthorized: caller must be Curator or Admin")]
fn test_blacklist_panics_if_caller_is_plain_user() {
    let (env, client) = setup();

    let user = Address::generate(&env);
    let target = Address::generate(&env);

    register(&client, &user, Role::User);
    register(&client, &target, Role::User);

    client.blacklist_user(&user, &target);
}

#[test]
#[should_panic(expected = "Caller profile not found")]
fn test_blacklist_panics_if_caller_not_registered() {
    let (env, client) = setup();

    let ghost = Address::generate(&env);
    let target = Address::generate(&env);

    register(&client, &target, Role::User);

    client.blacklist_user(&ghost, &target);
}
