#![no_std]

use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, Address, Env, String, Symbol,
};

// --- Types ---

#[contracttype]
#[derive(Clone)]
pub enum UserRole {
    Finder,
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
}

// --- Events ---

#[contractevent]
pub struct ProfileUpdated {
    #[topic]
    pub user: Address,
    pub metadata_hash: String,
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

// --- Contract ---

#[contract]
pub struct Registry;

#[contractimpl]
impl Registry {
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

    pub fn update_profile_metadata(env: Env, user: Address, new_metadata_hash: String) {
        user.require_auth();

        let mut profile = match read_profile(&env, &user) {
            Some(p) => p,
            None => panic!("User not registered"),
        };

        profile.metadata_hash = new_metadata_hash.clone();
        write_profile(&env, &user, &profile);

        ProfileUpdated {
            user,
            metadata_hash: new_metadata_hash,
        }
        .publish(&env);
    }
}

mod test;
