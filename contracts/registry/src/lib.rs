#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

/// Error codes for the Registry contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// User not found in registry
    UserNotFound = 1,
    /// User already exists
    UserAlreadyExists = 2,
}

/// User profile data structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserProfile {
    /// User's role in the system
    pub role: String,
    /// Verification status (true if verified, false otherwise)
    pub verified: bool,
    /// User's badge or tier level
    pub badge: String,
}

/// Storage keys for the contract
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Key for storing user profiles
    User(Address),
}

#[contract]
pub struct RegistryContract;

#[contractimpl]
impl RegistryContract {
    /// Registers a new user profile in the registry
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `user` - The address of the user to register
    /// * `role` - The user's role
    /// * `verified` - Verification status
    /// * `badge` - The user's badge
    ///
    /// # Returns
    /// * `Result<(), RegistryError>` - Ok if successful, UserAlreadyExists error if user exists
    pub fn register_user(
        env: Env,
        user: Address,
        role: String,
        verified: bool,
        badge: String,
    ) -> Result<(), RegistryError> {
        // Require authentication from the user being registered
        user.require_auth();

        // Construct the storage key
        let key = DataKey::User(user.clone());

        // Check if user already exists
        if env.storage().persistent().has(&key) {
            return Err(RegistryError::UserAlreadyExists);
        }

        // Create the user profile
        let profile = UserProfile {
            role,
            verified,
            badge,
        };

        // Store the profile in persistent storage
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Retrieves a user's profile from the registry (View Function)
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `user` - The address of the user to retrieve
    ///
    /// # Returns
    /// * `Result<UserProfile, RegistryError>` - The user profile if found, UserNotFound error otherwise
    pub fn get_profile(env: Env, user: Address) -> Result<UserProfile, RegistryError> {
        // 1. Construct DataKey::User(user_address)
        let key = DataKey::User(user);

        // 2. Call env.storage().persistent().get(&key)
        let profile: Option<UserProfile> = env.storage().persistent().get(&key);

        // 3. Unwrap result (or return error if None)
        match profile {
            Some(profile) => Ok(profile),
            None => Err(RegistryError::UserNotFound),
        }
    }

    /// Updates verification status for an existing user
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `user` - The address of the user to update
    /// * `verified` - New verification status
    ///
    /// # Returns
    /// * `Result<(), RegistryError>` - Ok if successful, UserNotFound if user doesn't exist
    pub fn update_verification(
        env: Env,
        user: Address,
        verified: bool,
    ) -> Result<(), RegistryError> {
        // Require authentication from the user
        user.require_auth();

        // Get the existing profile
        let key = DataKey::User(user.clone());
        let mut profile: UserProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::UserNotFound)?;

        // Update verification status
        profile.verified = verified;

        // Save the updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Updates the role for an existing user
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `user` - The address of the user to update
    /// * `role` - New role
    ///
    /// # Returns
    /// * `Result<(), RegistryError>` - Ok if successful, UserNotFound if user doesn't exist
    pub fn update_role(env: Env, user: Address, role: String) -> Result<(), RegistryError> {
        // Require authentication from the user
        user.require_auth();

        // Get the existing profile
        let key = DataKey::User(user.clone());
        let mut profile: UserProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::UserNotFound)?;

        // Update role
        profile.role = role;

        // Save the updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }

    /// Updates the badge for an existing user
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `user` - The address of the user to update
    /// * `badge` - New badge
    ///
    /// # Returns
    /// * `Result<(), RegistryError>` - Ok if successful, UserNotFound if user doesn't exist
    pub fn update_badge(env: Env, user: Address, badge: String) -> Result<(), RegistryError> {
        // Require authentication from the user
        user.require_auth();

        // Get the existing profile
        let key = DataKey::User(user.clone());
        let mut profile: UserProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::UserNotFound)?;

        // Update badge
        profile.badge = badge;

        // Save the updated profile
        env.storage().persistent().set(&key, &profile);

        Ok(())
    }
}

mod test;
