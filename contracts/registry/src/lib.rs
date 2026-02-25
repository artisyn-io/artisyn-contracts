#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env, String};

pub const ROLE_FINDER: u32 = 0;
pub const ROLE_CURATOR: u32 = 1;
pub const ROLE_ADMIN: u32 = 2;
pub const ROLE_ARTISAN: u32 = 3;

#[derive(Clone)]
#[contracttype]
pub struct Profile {
    pub role: u32,
    pub metadata_hash: String,
    pub is_verified: bool,
    pub is_blacklisted: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Profile(Address),
    Admin,
}

#[contractevent]
pub struct UserRegistered {
    #[topic]
    pub user: Address,
    pub role: u32,
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

#[contractevent]
pub struct UserVerified {
    #[topic]
    pub artisan: Address,
}

#[contractevent]
pub struct ApplicationReceived {
    #[topic]
    pub user_address: Address,
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
    pub fn initialize(env: Env, admin: Address) {
        if read_admin(&env).is_some() {
            panic!("Already initialized");
        }
        write_admin(&env, &admin);
    }

    pub fn register_user(env: Env, user: Address, metadata_hash: String) {
        user.require_auth();

        if read_profile(&env, &user).is_some() {
            panic!("User already registered");
        }

        let profile = Profile {
            role: ROLE_FINDER,
            metadata_hash,
            is_verified: false,
            is_blacklisted: false,
        };

        write_profile(&env, &user, &profile);

        UserRegistered {
            user,
            role: ROLE_FINDER,
        }
        .publish(&env);
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

    pub fn remove_curator(env: Env, curator: Address) {
        let admin = read_admin(&env).expect("Contract not initialized");
        admin.require_auth();

        let mut profile = match read_profile(&env, &curator) {
            Some(p) => p,
            None => panic!("User not found"),
        };

        if profile.role != ROLE_CURATOR {
            panic!("User is not a Curator");
        }

        profile.role = ROLE_FINDER;
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

    pub fn apply_for_verification(env: Env, caller: Address) {
        caller.require_auth();

        let profile = match read_profile(&env, &caller) {
            Some(p) => p,
            None => panic!("User not registered"),
        };

        if profile.metadata_hash.is_empty() {
            panic!("Metadata hash is missing");
        }

        ApplicationReceived {
            user_address: caller,
        }
        .publish(&env);
    }

    pub fn approve_artisan(env: Env, caller: Address, artisan: Address) {
        caller.require_auth();

        let caller_profile = match read_profile(&env, &caller) {
            Some(p) => p,
            None => panic!("Caller not registered"),
        };

        if caller_profile.role != ROLE_CURATOR && caller_profile.role != ROLE_ADMIN {
            panic!("Caller must be Curator or Admin");
        }

        let mut artisan_profile = match read_profile(&env, &artisan) {
            Some(p) => p,
            None => panic!("User not found"),
        };

        artisan_profile.role = ROLE_ARTISAN;
        artisan_profile.is_verified = true;
        write_profile(&env, &artisan, &artisan_profile);

        UserVerified { artisan }.publish(&env);
    }
}

#[cfg(test)]
mod test;
