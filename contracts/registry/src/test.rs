#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_get_profile_returns_correct_data_for_registered_users() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    // Create a test user
    let user = Address::generate(&env);
    let role = String::from_str(&env, "Artist");
    let badge = String::from_str(&env, "Gold");
    let verified = true;

    // Mock the authentication for registration
    env.mock_all_auths();

    // Register the user
    client.register_user(&user, &role, &verified, &badge);

    // Retrieve the profile using get_profile
    let profile = client.get_profile(&user);

    // Verify the returned data matches what was registered
    assert_eq!(profile.role, role);
    assert_eq!(profile.verified, verified);
    assert_eq!(profile.badge, badge);
}

#[test]
fn test_get_profile_returns_error_for_non_registered_users() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    // Create a test user that is NOT registered
    let unregistered_user = Address::generate(&env);

    // Attempt to retrieve the profile
    let result = client.try_get_profile(&unregistered_user);

    // Verify that it returns an error
    assert!(result.is_err());

    // Verify it's the correct error type
    let error = result.unwrap_err();
    assert_eq!(error.unwrap(), RegistryError::UserNotFound);
}

#[test]
fn test_register_user_creates_profile() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let role = String::from_str(&env, "Collector");
    let badge = String::from_str(&env, "Silver");
    let verified = false;

    env.mock_all_auths();

    // Register user
    client.register_user(&user, &role, &verified, &badge);

    // Verify profile exists and has correct data
    let profile = client.get_profile(&user);
    assert_eq!(profile.role, role);
    assert_eq!(profile.verified, verified);
    assert_eq!(profile.badge, badge);
}

#[test]
fn test_register_user_fails_if_already_exists() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let role = String::from_str(&env, "Developer");
    let badge = String::from_str(&env, "Bronze");

    env.mock_all_auths();

    // Register user first time
    client.register_user(&user, &role, &false, &badge);

    // Try to register same user again
    let result = client.try_register_user(&user, &role, &true, &badge);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert_eq!(error.unwrap(), RegistryError::UserAlreadyExists);
}

#[test]
fn test_get_profile_with_different_verification_states() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    env.mock_all_auths();

    // Test with verified user
    let verified_user = Address::generate(&env);
    client.register_user(
        &verified_user,
        &String::from_str(&env, "VerifiedArtist"),
        &true,
        &String::from_str(&env, "Platinum"),
    );

    let profile = client.get_profile(&verified_user);
    assert!(profile.verified);

    // Test with unverified user
    let unverified_user = Address::generate(&env);
    client.register_user(
        &unverified_user,
        &String::from_str(&env, "NewArtist"),
        &false,
        &String::from_str(&env, "None"),
    );

    let profile = client.get_profile(&unverified_user);
    assert!(!profile.verified);
}

#[test]
fn test_update_verification_status() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    env.mock_all_auths();

    // Register user as unverified
    client.register_user(
        &user,
        &String::from_str(&env, "Artist"),
        &false,
        &String::from_str(&env, "Bronze"),
    );

    // Verify initial state
    let profile = client.get_profile(&user);
    assert!(!profile.verified);

    // Update verification status
    client.update_verification(&user, &true);

    // Verify updated state
    let profile = client.get_profile(&user);
    assert!(profile.verified);
}

#[test]
fn test_update_role() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let initial_role = String::from_str(&env, "Artist");
    let new_role = String::from_str(&env, "Curator");

    env.mock_all_auths();

    // Register user
    client.register_user(&user, &initial_role, &true, &String::from_str(&env, "Gold"));

    // Update role
    client.update_role(&user, &new_role);

    // Verify updated role
    let profile = client.get_profile(&user);
    assert_eq!(profile.role, new_role);
}

#[test]
fn test_update_badge() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let initial_badge = String::from_str(&env, "Bronze");
    let new_badge = String::from_str(&env, "Platinum");

    env.mock_all_auths();

    // Register user
    client.register_user(
        &user,
        &String::from_str(&env, "Collector"),
        &true,
        &initial_badge,
    );

    // Update badge
    client.update_badge(&user, &new_badge);

    // Verify updated badge
    let profile = client.get_profile(&user);
    assert_eq!(profile.badge, new_badge);
}

#[test]
fn test_multiple_users_independent_profiles() {
    let env = Env::default();
    let contract_id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &contract_id);

    env.mock_all_auths();

    // Register multiple users
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    client.register_user(
        &user1,
        &String::from_str(&env, "Artist"),
        &true,
        &String::from_str(&env, "Gold"),
    );

    client.register_user(
        &user2,
        &String::from_str(&env, "Collector"),
        &false,
        &String::from_str(&env, "Silver"),
    );

    client.register_user(
        &user3,
        &String::from_str(&env, "Curator"),
        &true,
        &String::from_str(&env, "Platinum"),
    );

    // Verify each user has their own independent profile
    let profile1 = client.get_profile(&user1);
    assert_eq!(profile1.role, String::from_str(&env, "Artist"));
    assert!(profile1.verified);
    assert_eq!(profile1.badge, String::from_str(&env, "Gold"));

    let profile2 = client.get_profile(&user2);
    assert_eq!(profile2.role, String::from_str(&env, "Collector"));
    assert!(!profile2.verified);
    assert_eq!(profile2.badge, String::from_str(&env, "Silver"));

    let profile3 = client.get_profile(&user3);
    assert_eq!(profile3.role, String::from_str(&env, "Curator"));
    assert!(profile3.verified);
    assert_eq!(profile3.badge, String::from_str(&env, "Platinum"));
}

fn setup_env() -> (Env, RegistryClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);
    (env, client)
}

/// Register a bare profile directly via persistent storage so tests that don't
/// care about `register_user` aren't blocked by a missing helper on the client.
fn seed_profile(env: &Env, contract_id: &Address, user: &Address, role: u32) {
    env.as_contract(contract_id, || {
        write_profile(
            env,
            user,
            &Profile {
                role,
                metadata_hash: String::from_str(env, "hash"),
                is_verified: false,
            },
        );
    });
}

/// Happy path: a Curator is successfully demoted to Finder.
#[test]
fn test_remove_curator_demotes_curator_to_finder() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let curator = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract with admin
    client.initialize(&admin);

    // Give the target user a Curator profile
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);

    // Verify starting role is Curator
    let profile_before = client.get_profile(&curator);
    assert_eq!(profile_before.role, ROLE_CURATOR);

    // Admin removes curator
    client.remove_curator(&curator);

    // Verify role has been downgraded to Finder
    let profile_after = client.get_profile(&curator);
    assert_eq!(profile_after.role, ROLE_FINDER, "Role should revert to Finder");
}

/// remove_curator must panic when the target user is not registered.
#[test]
#[should_panic(expected = "User not found")]
fn test_remove_curator_panics_for_unregistered_user() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let ghost = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    // ghost has no profile — should panic
    client.remove_curator(&ghost);
}

/// remove_curator must panic when the target user's role is not Curator.
#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_remove_curator_panics_if_not_curator() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    // finder has a Finder role — not Curator
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    client.remove_curator(&finder);
}

/// remove_curator must not affect other users' profiles.
#[test]
fn test_remove_curator_does_not_affect_other_users() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let curator1 = Address::generate(&env);
    let curator2 = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    seed_profile(&env, &contract_id, &curator1, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &curator2, ROLE_CURATOR);

    // Only demote curator1
    client.remove_curator(&curator1);

    assert_eq!(client.get_profile(&curator1).role, ROLE_FINDER);
    assert_eq!(
        client.get_profile(&curator2).role,
        ROLE_CURATOR,
        "curator2 must remain untouched"
    );
}

/// Calling remove_curator on an already-demoted user must panic.
#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_remove_curator_cannot_be_called_twice() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let curator = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    client.remove_curator(&curator); // first call succeeds
    client.remove_curator(&curator); // second call must panic
}

/// Admin role itself must not be demoteable via remove_curator.
#[test]
#[should_panic(expected = "User is not a Curator")]
fn test_remove_curator_cannot_demote_admin() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    seed_profile(&env, &contract_id, &admin, ROLE_ADMIN);

    // Attempt to demote the admin — must fail because role != Curator
    client.remove_curator(&admin);
}

#[test]
fn test_get_profile_returns_error_for_non_registered_users() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    let unregistered_user = Address::generate(&env);
    let result = client.try_get_profile(&unregistered_user);
    assert!(result.is_err());
}

#[test]
fn test_multiple_users_independent_profiles() {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);

    env.mock_all_auths();

    let u1 = Address::generate(&env);
    let u2 = Address::generate(&env);

    seed_profile(&env, &contract_id, &u1, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &u2, ROLE_FINDER);

    assert_eq!(client.get_profile(&u1).role, ROLE_CURATOR);
    assert_eq!(client.get_profile(&u2).role, ROLE_FINDER);
}