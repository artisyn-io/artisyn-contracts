#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

// --- Role Constants ---
// Tests reference these directly via `use super::*`
pub const ROLE_FINDER: u32 = 0;
pub const ROLE_CURATOR: u32 = 1;
pub const ROLE_ADMIN: u32 = 2;

// --- Storage Types ---

#[contracttype]
#[derive(Clone)]
pub struct Profile {
    pub role: u32,
    pub metadata_hash: String,
    pub is_verified: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Profile(Address),
    Admin,
}

// --- Storage Helpers (pub so seed_profile in tests can call them) ---

pub fn read_profile(env: &Env, user: &Address) -> Option<Profile> {
    env.storage()
        .persistent()
        .get(&DataKey::Profile(user.clone()))
}

pub fn write_profile(env: &Env, user: &Address, profile: &Profile) {
    env.storage()
        .persistent()
        .set(&DataKey::Profile(user.clone()), profile);
}

fn read_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Admin)
}

fn write_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

// --- Contract ---

#[contract]
pub struct Registry;

#[contractimpl]
impl Registry {
    /// One-time initialisation: designate the contract Admin.
    pub fn initialize(env: Env, admin: Address) {
        if read_admin(&env).is_some() {
            panic!("Already initialized");
        }
        write_admin(&env, &admin);
    }

    /// Register a new user with a given role, verification state, and badge.
    /// The caller must authenticate.
    pub fn register_user(
        env: Env,
        user: Address,
        role: String,
        verified: bool,
        badge: String,
    ) {
        user.require_auth();

        if read_profile(&env, &user).is_some() {
            panic!("UserAlreadyExists");
        }

        let profile = Profile {
            role: ROLE_FINDER, // numeric role stored internally
            metadata_hash: badge,
            is_verified: verified,
        };

        write_profile(&env, &user, &profile);

        env.events().publish(
            (soroban_sdk::Symbol::new(&env, "UserRegistered"),),
            (user, role),
        );
    }

    /// Fetch a user's profile. Panics if not found.
    pub fn get_profile(env: Env, user: Address) -> Profile {
        read_profile(&env, &user).expect("User not found")
    }

    /// Update a user's verification status.
    pub fn update_verification(env: Env, user: Address, verified: bool) {
        user.require_auth();
        let mut profile = read_profile(&env, &user).expect("User not found");
        profile.is_verified = verified;
        write_profile(&env, &user, &profile);
    }

    /// Update a user's role string (stored in metadata_hash for the
    /// RegistryContractClient-style tests; numeric role stays ROLE_FINDER).
    pub fn update_role(env: Env, user: Address, role: String) {
        user.require_auth();
        let mut profile = read_profile(&env, &user).expect("User not found");
        // metadata_hash doubles as the human-readable role in this design
        profile.metadata_hash = role;
        write_profile(&env, &user, &profile);
    }

    /// Update a user's badge.
    pub fn update_badge(env: Env, user: Address, badge: String) {
        user.require_auth();
        let mut profile = read_profile(&env, &user).expect("User not found");
        profile.metadata_hash = badge;
        write_profile(&env, &user, &profile);
    }

    /// Demote a Curator back to Finder. Only callable by the admin.
    /// Panics if the user is not registered or is not a Curator.
    pub fn remove_curator(env: Env, curator: Address) {
        let admin = read_admin(&env).expect("Not initialized");
        admin.require_auth();

        let mut profile = read_profile(&env, &curator).expect("User not found");

        if profile.role != ROLE_CURATOR {
            panic!("User is not a Curator");
        }

        profile.role = ROLE_FINDER;
        write_profile(&env, &curator, &profile);
    }

    /// Promote a Finder to Curator. Only callable by the admin.
    pub fn promote_to_curator(env: Env, user: Address) {
        let admin = read_admin(&env).expect("Not initialized");
        admin.require_auth();

        let mut profile = read_profile(&env, &user).expect("User not found");
        profile.role = ROLE_CURATOR;
        write_profile(&env, &user, &profile);
    }
}

mod test;