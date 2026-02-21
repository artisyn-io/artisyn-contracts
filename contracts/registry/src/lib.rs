#![no_std]

use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env, String};

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum UserRole {
    User,
    Curator,
}

#[derive(Clone)]
#[contracttype]
pub struct UserProfile {
    pub role: UserRole,
    pub metadata_hash: String,
    pub is_verified: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    User(Address),
}

#[contractevent]
pub struct ProfileUpdated {
    #[topic]
    pub user: Address,
    pub metadata_hash: String,
}

#[contract]
pub struct Registry;

fn read_profile(env: &Env, user: &Address) -> Option<UserProfile> {
    env.storage().persistent().get(&DataKey::User(user.clone()))
}

fn write_profile(env: &Env, user: &Address, profile: &UserProfile) {
    env.storage()
        .persistent()
        .set(&DataKey::User(user.clone()), profile);
}

#[contractimpl]
impl Registry {
    /// Initialize the registry with the Admin address. Must be called once at deploy.
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Register a user (self-registration). Creates a profile with User role.
    pub fn register_user(env: Env, user: Address) {
        user.require_auth();
        if read_profile(&env, &user).is_some() {
            panic!("User already registered");
        }
        let profile = UserProfile {
            role: UserRole::User,
            metadata_hash: String::from_str(&env, ""),
            is_verified: false,
        };
        write_profile(&env, &user, &profile);
    }

    /// Promote a trusted user to Curator. Only the Admin can call this.
    pub fn add_curator(env: Env, new_curator: Address) {
        let admin = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"));

        admin.require_auth();

        let mut profile: UserProfile = env
            .storage()
            .persistent()
            .get(&DataKey::User(new_curator.clone()))
            .unwrap_or_else(|| panic!("User not found"));

        profile.role = UserRole::Curator;
        env.storage()
            .persistent()
            .set(&DataKey::User(new_curator), &profile);
    }

    /// Return the profile for a user, or panic if not found.
    pub fn get_profile(env: Env, user: Address) -> UserProfile {
        read_profile(&env, &user).unwrap_or_else(|| panic!("User not found"))
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
