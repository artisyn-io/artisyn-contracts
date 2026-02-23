use super::*;
use soroban_sdk::{testutils::Address as _, Env};

fn setup_env() -> (Env, Address, RegistryClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(Registry, ());
    let client = RegistryClient::new(&env, &contract_id);
    (env, contract_id, client)
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
    let (env, contract_id, client) = setup_env();
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
    assert_eq!(
        profile_after.role, ROLE_FINDER,
        "Role should revert to Finder"
    );
}

/// remove_curator must panic when the target user is not registered.
#[test]
#[should_panic(expected = "User not found")]
fn test_remove_curator_panics_for_unregistered_user() {
    let (env, _contract_id, client) = setup_env();
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
    let (env, contract_id, client) = setup_env();
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
    let (env, contract_id, client) = setup_env();
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
    let (env, contract_id, client) = setup_env();
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
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    seed_profile(&env, &contract_id, &admin, ROLE_ADMIN);

    // Attempt to demote the admin — must fail because role != Curator
    client.remove_curator(&admin);
}

/// Happy path: Curator successfully approves a Finder to become an Artisan.
#[test]
fn test_approve_artisan_by_curator() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract with admin
    client.initialize(&admin);

    // Create curator and finder profiles
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    // Verify starting role is Finder
    let profile_before = client.get_profile(&finder);
    assert_eq!(profile_before.role, ROLE_FINDER);

    // Curator approves the finder to become an artisan
    client.approve_artisan(&curator, &finder);

    // Verify role has been upgraded to Artisan
    let profile_after = client.get_profile(&finder);
    assert_eq!(
        profile_after.role, ROLE_ARTISAN,
        "Role should be upgraded to Artisan"
    );
}

/// Happy path: Admin successfully approves a Finder to become an Artisan.
#[test]
fn test_approve_artisan_by_admin() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract with admin
    client.initialize(&admin);

    // Create admin and finder profiles
    seed_profile(&env, &contract_id, &admin, ROLE_ADMIN);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    // Verify starting role is Finder
    let profile_before = client.get_profile(&finder);
    assert_eq!(profile_before.role, ROLE_FINDER);

    // Admin approves the finder to become an artisan
    client.approve_artisan(&admin, &finder);

    // Verify role has been upgraded to Artisan
    let profile_after = client.get_profile(&finder);
    assert_eq!(
        profile_after.role, ROLE_ARTISAN,
        "Role should be upgraded to Artisan"
    );
}

/// approve_artisan must panic when called by a Finder (non-privileged user).
#[test]
#[should_panic(expected = "Caller must be Curator or Admin")]
fn test_approve_artisan_panics_when_called_by_finder() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let finder1 = Address::generate(&env);
    let finder2 = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract
    client.initialize(&admin);

    // Create two finder profiles
    seed_profile(&env, &contract_id, &finder1, ROLE_FINDER);
    seed_profile(&env, &contract_id, &finder2, ROLE_FINDER);

    // Attempt to approve artisan as a Finder — should panic
    client.approve_artisan(&finder1, &finder2);
}

/// approve_artisan must panic when the target user is not registered.
#[test]
#[should_panic(expected = "User not found")]
fn test_approve_artisan_panics_for_unregistered_user() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let ghost = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract
    client.initialize(&admin);

    // Create curator profile
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);

    // ghost has no profile — should panic
    client.approve_artisan(&curator, &ghost);
}

/// approve_artisan must not affect other users' profiles.
#[test]
fn test_approve_artisan_does_not_affect_other_users() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder1 = Address::generate(&env);
    let finder2 = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract
    client.initialize(&admin);

    // Create profiles
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder1, ROLE_FINDER);
    seed_profile(&env, &contract_id, &finder2, ROLE_FINDER);

    // Only approve finder1
    client.approve_artisan(&curator, &finder1);

    // Verify finder1 is now Artisan
    assert_eq!(client.get_profile(&finder1).role, ROLE_ARTISAN);

    // Verify finder2 remains unchanged
    assert_eq!(
        client.get_profile(&finder2).role,
        ROLE_FINDER,
        "finder2 must remain untouched"
    );

    // Verify curator remains unchanged
    assert_eq!(
        client.get_profile(&curator).role,
        ROLE_CURATOR,
        "curator must remain untouched"
    );
}

/// approve_artisan can be called on an already-approved Artisan (idempotent).
#[test]
fn test_approve_artisan_is_idempotent() {
    let (env, contract_id, client) = setup_env();
    let admin = Address::generate(&env);
    let curator = Address::generate(&env);
    let finder = Address::generate(&env);

    env.mock_all_auths();

    // Initialize contract
    client.initialize(&admin);

    // Create profiles
    seed_profile(&env, &contract_id, &curator, ROLE_CURATOR);
    seed_profile(&env, &contract_id, &finder, ROLE_FINDER);

    // First approval
    client.approve_artisan(&curator, &finder);
    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);

    // Second approval should succeed (idempotent)
    client.approve_artisan(&curator, &finder);
    assert_eq!(client.get_profile(&finder).role, ROLE_ARTISAN);
}
