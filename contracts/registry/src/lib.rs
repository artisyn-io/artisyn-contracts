#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
pub enum DataKey {
    UserProfile(Address),
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum Role {
    User,
    Curator,
    Admin,
}

#[contracttype]
#[derive(Clone)]
pub struct UserProfile {
    pub address: Address,
    pub role: Role,
    pub is_blacklisted: bool,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[contractevent]
pub struct UserBlacklisted {
    pub target_user: Address,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct RegistryContract;

#[contractimpl]
impl RegistryContract {
    /// Register a user profile. Stores the profile in persistent storage.
    /// In production this would be protected; here it is open for bootstrapping.
    pub fn register_user(env: Env, user: Address, role: Role) {
        user.require_auth();
        let profile = UserProfile {
            address: user.clone(),
            role,
            is_blacklisted: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::UserProfile(user), &profile);
    }

    /// Blacklist a user. Caller must be a Curator or Admin.
    ///
    /// 1. Require auth from caller.
    /// 2. Load caller profile — assert role is Curator or Admin.
    /// 3. Load target profile — panic if not found.
    /// 4. Set `is_blacklisted = true`.
    /// 5. Save updated profile to persistent storage.
    /// 6. Emit `UserBlacklisted` event.
    pub fn blacklist_user(env: Env, caller: Address, target_user: Address) {
        // 1. Require auth from the caller
        caller.require_auth();

        // 2. Load caller profile and verify role
        let caller_profile: UserProfile = env
            .storage()
            .persistent()
            .get(&DataKey::UserProfile(caller.clone()))
            .expect("Caller profile not found");

        if caller_profile.role != Role::Curator && caller_profile.role != Role::Admin {
            panic!("Unauthorized: caller must be Curator or Admin");
        }

        // 3. Load target profile — panic if not found
        let mut target_profile: UserProfile = env
            .storage()
            .persistent()
            .get(&DataKey::UserProfile(target_user.clone()))
            .expect("Target user profile not found");

        // 4. Set is_blacklisted = true
        target_profile.is_blacklisted = true;

        // 5. Save updated profile
        env.storage()
            .persistent()
            .set(&DataKey::UserProfile(target_user.clone()), &target_profile);

        // 6. Emit UserBlacklisted event
        UserBlacklisted {
            target_user: target_user.clone(),
        }
        .publish(&env);
    }

    /// Returns the profile for the given address.
    pub fn get_profile(env: Env, user: Address) -> UserProfile {
        env.storage()
            .persistent()
            .get(&DataKey::UserProfile(user))
            .expect("Profile not found")
    }
}

mod test;
