#![no_std]

use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, Address, Env, String, Symbol,
};

// --- Types ---

pub const ROLE_FINDER: u32 = 0;
pub const ROLE_CURATOR: u32 = 1;
pub const ROLE_ADMIN: u32 = 2;

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

// --- Events ---

#[contractevent]
pub struct ProfileUpdated {
    #[topic]
    pub user: Address,
    pub metadata_hash: String,
}

#[contractevent]
pub struct CuratorRemoved {
    #[topic]
    pub curator: Address,
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

    /// Register a new user as a Finder.
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

    /// Update a user's metadata hash (user-gated).
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

    /// Promote a user to Curator (admin-gated).
    pub fn add_curator(env: Env, curator: Address) {
        let admin = read_admin(&env).expect("Contract not initialized");
        admin.require_auth();

        let mut profile = match read_profile(&env, &curator) {
            Some(p) => p,
            None => panic!("User not found"),
        };

        if profile.role == UserRole::Curator {
            panic!("User is already a Curator");
        }

        profile.role = UserRole::Curator;
        write_profile(&env, &curator, &profile);
    }

    /// Demote a Curator back to Finder (admin-gated).
    pub fn remove_curator(env: Env, curator: Address) {
        let admin = read_admin(&env).expect("Contract not initialized");
        admin.require_auth();

        let mut profile = match read_profile(&env, &curator) {
            Some(p) => p,
            None => panic!("User not found"),
        };

        if profile.role != UserRole::Curator {
            panic!("User is not a Curator");
        }

        profile.role = UserRole::Finder;
        write_profile(&env, &curator, &profile);

        CuratorRemoved { curator }.publish(&env);
    }

    pub fn get_profile(env: Env, user: Address) -> UserProfile {
        match read_profile(&env, &user) {
            Some(p) => p,
            None => panic!("User not found"),
        }
    }

    pub fn get_admin(env: Env) -> Address {
        read_admin(&env).expect("Contract not initialized")
    }
}

mod test;