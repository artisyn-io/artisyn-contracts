#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, token, Address, Env};

// Import the registry contract client for cross-contract calls
mod registry {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/registry.wasm"
    );
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobStatus {
    Open,
    Assigned,
    InProgress,
    PendingReview,
    Completed,
    Disputed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    pub id: u64,
    pub finder: Address,
    pub artisan: Option<Address>,
    pub token: Address,
    pub amount: i128,
    pub status: JobStatus,
    pub start_time: u64,
    pub end_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Job(u64),
    JobCounter,
    RegistryContract,
}

#[contractevent]
pub struct JobCreated {
    pub id: u64,
    pub amount: i128,
}

#[contractevent]
pub struct JobAssigned {
    pub id: u64,
    pub artisan: Address,
}

#[contract]
pub struct MarketContract;

#[contractimpl]
impl MarketContract {
    /// Initialize the market contract with the registry contract address.
    /// Must be called once before using the contract.
    pub fn initialize(env: Env, registry_contract: Address) {
        if env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::RegistryContract)
            .is_some()
        {
            panic!("Already initialized");
        }
        env.storage()
            .instance()
            .set(&DataKey::RegistryContract, &registry_contract);
    }

    pub fn create_job(env: Env, finder: Address, token: Address, amount: i128) -> u64 {
        // 1. Require auth from the finder
        finder.require_auth();

        // 2. Transfer token from finder to this contract
        let token_client = token::TokenClient::new(&env, &token);
        token_client.transfer(&finder, env.current_contract_address(), &amount);

        // 3. Get and increment job counter
        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::JobCounter)
            .unwrap_or(0);
        let id = counter + 1;
        env.storage().instance().set(&DataKey::JobCounter, &id);

        // 4. Store job in persistent storage
        let job = Job {
            id,
            finder,
            artisan: None,
            token,
            amount,
            status: JobStatus::Open,
            start_time: 0, // Set to 0, will be updated when an artisan starts the job
            end_time: 0,   // Set to 0, will be updated when the job is completed
        };
        env.storage().persistent().set(&DataKey::Job(id), &job);

        // 5. Emit JobCreated event
        JobCreated { id, amount }.publish(&env);

        // 6. Return job id
        id
    }

    /// Assign a verified Artisan to an open job.
    ///
    /// # Panics
    /// - If the contract has not been initialized
    /// - If the job does not exist
    /// - If the job status is not Open
    /// - If the caller is not the Finder who created the job
    /// - If the artisan is not verified in the Registry contract
    pub fn assign_artisan(env: Env, job_id: u64, artisan: Address) {
        // 1. Get registry contract address
        let registry_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::RegistryContract)
            .expect("Contract not initialized");

        // 2. Retrieve the job
        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        // 3. Require auth from the Finder
        job.finder.require_auth();

        // 4. Ensure job status is Open
        if job.status != JobStatus::Open {
            panic!("Job is not open");
        }

        // 5. Cross-contract call to Registry to verify artisan role
        let registry_client = registry::Client::new(&env, &registry_contract);
        let profile = registry_client.get_profile(&artisan);

        // Verify the user has the Artisan role (role = 3)
        if profile.role != 3 {
            panic!("User is not a verified Artisan");
        }

        // 6. Update job with artisan and change status to Assigned
        job.artisan = Some(artisan.clone());
        job.status = JobStatus::Assigned;

        // 7. Save updated job
        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        // 8. Emit JobAssigned event
        JobAssigned {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }
}

mod test;
