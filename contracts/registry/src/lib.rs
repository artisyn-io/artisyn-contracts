#![no_std]

use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env, String};

// Using u32 to stay consistent with the existing Profile struct.
pub const ROLE_FINDER: u32 = 0;
pub const ROLE_CURATOR: u32 = 1;
pub const ROLE_ADMIN: u32 = 2;

#[derive(Clone)]
#[contracttype]
pub struct Profile {
    pub role: u32,
    pub metadata_hash: String,
    pub is_verified: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Profile(Address),
    Admin,
}

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

#[contract]
pub struct Registry;

fn read_profile(env: &Env, user: &Address) -> Option<Profile> {
    env.storage()
        .persistent()
        .get(&DataKey::Profile(user.clone()))
}

fn write_profile(env: &Env, user: &Address, profile: &Profile) {
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

#[contractimpl]
impl Registry {
    /// One-time initialisation: designate the contract Admin.
    /// Must be called before any admin-gated functions.
    pub fn initialize(env: Env, admin: Address) {
        if read_admin(&env).is_some() {
            panic!("Already initialized");
        }
        write_admin(&env, &admin);
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

        if profile.role == ROLE_CURATOR {
            panic!("User is already a Curator");
        }

        profile.role = ROLE_CURATOR;
        write_profile(&env, &curator, &profile);
    }

    /// Demote a Curator back to Finder (admin-gated).
    ///
    /// # Panics
    /// - If the contract has not been initialized (no admin set)
    /// - If `curator` has no registered profile
    /// - If `curator`'s current role is not `Curator`
    pub fn remove_curator(env: Env, curator: Address) {
        // 1. Verify caller is Admin
        let admin = read_admin(&env).expect("Contract not initialized");
        admin.require_auth();

        // 2. Retrieve target user's profile — panic if not found
        let mut profile = match read_profile(&env, &curator) {
            Some(p) => p,
            None => panic!("User not found"),
        };

        // 3. Ensure target currently has role Curator — panic if not
        if profile.role != ROLE_CURATOR {
            panic!("User is not a Curator");
        }

        // 4. Downgrade role to Finder (safe default)
        profile.role = ROLE_FINDER;

        // 5. Save updated profile
        write_profile(&env, &curator, &profile);

        CuratorRemoved { curator }.publish(&env);
    }

    pub fn get_profile(env: Env, user: Address) -> Profile {
        match read_profile(&env, &user) {
            Some(p) => p,
            None => panic!("User not found"),
        }
    }

    pub fn get_admin(env: Env) -> Address {
        read_admin(&env).expect("Contract not initialized")
    }
}