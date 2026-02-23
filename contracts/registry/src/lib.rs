#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    Env, Address, String, Symbol,
};

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum UserRole {
    Finder,
    Curator,
    Admin,
}

#[contracttype]
#[derive(Clone)]
pub struct UserProfile {
    pub role: UserRole,
    pub metadata_hash: String,
    pub is_verified: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Profile(Address),
    Admin,
}

// --- Storage Helpers ---

fn read_profile(env: &Env, user: &Address) -> Option<UserProfile> {
    env.storage()
        .persistent()
        .get(&DataKey::Profile(user.clone()))
}

fn write_profile(env: &Env, user: &Address, profile: &UserProfile) {
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

    /// Register a new user as a Finder..
    pub fn register(env: Env, user: Address, metadata_hash: String) {
        user.require_auth();

        if read_profile(&env, &user).is_some() {
            panic!("User already registered");
        }

        let profile = UserProfile {
            role: UserRole::Finder,
            metadata_hash: metadata_hash.clone(),
            is_verified: false,
        };

        write_profile(&env, &user, &profile);

        env.events().publish(
            (Symbol::new(&env, "UserRegistered"),),
            (user, metadata_hash),
        );
    }
}

mod test;