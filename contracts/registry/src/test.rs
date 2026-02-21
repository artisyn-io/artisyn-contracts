#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup(env: &Env) -> (Address, RegistryClient<'_>) {
    let admin = Address::generate(env);
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(env, &contract_id);
    client.init(&admin);
    (admin, client)
}

// ---------------------------------------------------------------------------
// add_curator — happy path
// ---------------------------------------------------------------------------

#[test]
fn test_add_curator_admin_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, client) = setup(&env);
    let new_curator = Address::generate(&env);

    client.register_user(&new_curator);
    assert_eq!(client.get_profile(&new_curator).role, UserRole::User);

    client.add_curator(&new_curator);

    let profile = client.get_profile(&new_curator);
    assert_eq!(profile.role, UserRole::Curator);
}

// ---------------------------------------------------------------------------
// add_curator — only Admin can call
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_add_curator_non_admin_panics() {
    let env = Env::default();
    // Do NOT use mock_all_auths: no address has authorized, so admin.require_auth() will fail.

    let (_admin, client) = setup(&env);
    let target = Address::generate(&env);

    client.register_user(&target);

    // No auth from admin → require_auth() in add_curator panics
    client.add_curator(&target);
}

// ---------------------------------------------------------------------------
// add_curator — target must exist
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "User not found")]
fn test_add_curator_user_not_found_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, client) = setup(&env);
    let ghost = Address::generate(&env);

    client.add_curator(&ghost);
}
