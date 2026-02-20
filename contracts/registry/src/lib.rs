#![no_std]

use soroban_sdk::{contract, contractimpl, Env, Address, String, Symbol};

#[contract]
pub struct Contract;

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

#[contractimpl]
impl Contract {
    pub fn register(env: Env, user: Address, metadata_hash: String) {
        // 1. Require authorization from the user
        user.require_auth();

        // 2. Check if the user already exists in Persistent storage
        let storage = env.storage().persistent();
        if storage.has(&user) {
            panic!("User already registered");
        }

        // 3. Create a new UserProfile struct with Role::Finder
        let profile = UserProfile {
            role: UserRole::Finder,
            metadata_hash: metadata_hash.clone(),
            is_verified: false,
        };

        // 4. Save the profile to Persistent storage using the user's Address as key
        storage.set(&user, &profile);

        // 5. Publish a 'UserRegistered' event with the user address and metadata hash
        env.events().publish((Symbol::new(&env, "UserRegistered"),), (user, metadata_hash));
    }
}

mod test;
