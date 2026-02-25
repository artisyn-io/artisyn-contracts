#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, token, Address, Env};

mod registry {
    use soroban_sdk::{contractclient, contracttype, Address, Env, String};

    #[contracttype]
    #[derive(Clone)]
    pub struct Profile {
        pub role: u32,
        pub metadata_hash: String,
        pub is_verified: bool,
        pub is_blacklisted: bool,
    }

    #[allow(dead_code)]
    #[contractclient(name = "Client")]
    pub trait RegistryTrait {
        fn get_profile(env: &Env, user: Address) -> Profile;
    }
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
    pub deadline: u64,
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

#[contractevent]
pub struct JobApplication {
    pub id: u64,
    pub artisan: Address,
}

#[contractevent]
pub struct JobStarted {
    pub id: u64,
    pub artisan: Address,
}

#[contractevent]
pub struct JobCancelled {
    pub id: u64,
}

#[contractevent]
pub struct JobCompleted {
    pub id: u64,
    pub artisan: Address,
}

#[contractevent]
pub struct FundsReleased {
    pub id: u64,
    pub artisan: Address,
    pub amount: i128,
}

#[contractevent]
pub struct DeadlineExtended {
    pub id: u64,
    pub extra_time: u64,
    pub new_deadline: u64,
}

#[contractevent]
pub struct BudgetIncreased {
    pub id: u64,
    pub added_amount: i128,
    pub new_amount: i128,
}

#[contract]
pub struct MarketContract;

#[contractimpl]
impl MarketContract {
    pub fn initialize(env: Env, registry_contract: Address) {
        if env.storage().instance().has(&DataKey::RegistryContract) {
            panic!("Already initialized");
        }
        env.storage()
            .instance()
            .set(&DataKey::RegistryContract, &registry_contract);
    }

    pub fn create_job(env: Env, finder: Address, token: Address, amount: i128) -> u64 {
        finder.require_auth();

        let token_client = token::TokenClient::new(&env, &token);
        token_client.transfer(&finder, env.current_contract_address(), &amount);

        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::JobCounter)
            .unwrap_or(0);
        let id = counter + 1;
        env.storage().instance().set(&DataKey::JobCounter, &id);

        let job = Job {
            id,
            finder,
            artisan: None,
            token,
            amount,
            status: JobStatus::Open,
            start_time: 0,
            end_time: 0,
            deadline: 0,
        };
        env.storage().persistent().set(&DataKey::Job(id), &job);

        JobCreated { id, amount }.publish(&env);

        id
    }

    pub fn assign_artisan(env: Env, finder: Address, job_id: u64, artisan: Address) {
        let registry_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::RegistryContract)
            .expect("Contract not initialized");

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        finder.require_auth();

        if job.finder != finder {
            panic!("Not job owner");
        }

        if job.status != JobStatus::Open {
            panic!("Job is not open");
        }

        let registry_client = registry::Client::new(&env, &registry_contract);
        let profile = registry_client.get_profile(&artisan);

        if profile.role != 3 {
            panic!("User is not a verified Artisan");
        }
        if profile.is_blacklisted {
            panic!("User is blacklisted");
        }

        job.artisan = Some(artisan.clone());
        job.status = JobStatus::Assigned;

        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        JobAssigned {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn apply_for_job(env: Env, artisan: Address, job_id: u64) {
        artisan.require_auth();

        let registry_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::RegistryContract)
            .expect("Contract not initialized");

        let job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.status != JobStatus::Open {
            panic!("Job is not open");
        }

        let registry_client = registry::Client::new(&env, &registry_contract);
        let profile = registry_client.get_profile(&artisan);

        if profile.role != 3 {
            panic!("User is not a verified Artisan");
        }
        if profile.is_blacklisted {
            panic!("User is blacklisted");
        }

        JobApplication {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn start_job(env: Env, artisan: Address, job_id: u64) {
        artisan.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.status != JobStatus::Assigned {
            panic!("Job is not assigned");
        }

        if job.artisan != Some(artisan.clone()) {
            panic!("Not assigned to this job");
        }

        job.status = JobStatus::InProgress;
        job.start_time = env.ledger().timestamp();

        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        JobStarted {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn cancel_job(env: Env, finder: Address, job_id: u64) {
        finder.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.finder != finder {
            panic!("Not job owner");
        }

        if job.status != JobStatus::Open {
            panic!("Job is not open");
        }

        let token_client = token::TokenClient::new(&env, &job.token);
        token_client.transfer(&env.current_contract_address(), &finder, &job.amount);

        job.status = JobStatus::Cancelled;

        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        JobCancelled { id: job_id }.publish(&env);
    }

    pub fn complete_job(env: Env, artisan: Address, job_id: u64) {
        artisan.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.artisan != Some(artisan.clone()) {
            panic!("Not assigned to this job");
        }

        if job.status != JobStatus::InProgress {
            panic!("Job is not in progress");
        }

        job.status = JobStatus::PendingReview;
        job.end_time = env.ledger().timestamp();

        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        JobCompleted {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn auto_release_funds(env: Env, artisan: Address, job_id: u64) {
        artisan.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.status != JobStatus::PendingReview {
            panic!("Job is not in PendingReview status");
        }

        let artisan_from_job = job.artisan.as_ref().expect("Job has no assigned artisan");
        if artisan_from_job != &artisan {
            panic!("Only the assigned artisan can release funds");
        }

        let current_time = env.ledger().timestamp();
        let seven_days_in_seconds: u64 = 604800;
        let release_time = job.end_time + seven_days_in_seconds;

        if current_time <= release_time {
            panic!("7 days have not passed since job completion");
        }

        let token_client = token::TokenClient::new(&env, &job.token);
        token_client.transfer(&env.current_contract_address(), &artisan, &job.amount);

        job.status = JobStatus::Completed;
        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        FundsReleased {
            id: job_id,
            artisan,
            amount: job.amount,
        }
        .publish(&env);
    }

    pub fn extend_deadline(env: Env, finder: Address, job_id: u64, extra_time: u64) {
        finder.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.finder != finder {
            panic!("Not job owner");
        }

        if job.status == JobStatus::Completed || job.status == JobStatus::Cancelled {
            panic!("Job is already finalized");
        }

        job.deadline += extra_time;

        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        DeadlineExtended {
            id: job_id,
            extra_time,
            new_deadline: job.deadline,
        }
        .publish(&env);
    }

    pub fn increase_budget(env: Env, finder: Address, job_id: u64, added_amount: i128) {
        finder.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.finder != finder {
            panic!("Not job owner");
        }

        if job.status == JobStatus::Completed || job.status == JobStatus::Cancelled {
            panic!("Job is already finalized");
        }

        let token_client = token::TokenClient::new(&env, &job.token);
        token_client.transfer(&finder, env.current_contract_address(), &added_amount);

        job.amount += added_amount;

        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        BudgetIncreased {
            id: job_id,
            added_amount,
            new_amount: job.amount,
        }
        .publish(&env);
    }
}

#[cfg(test)]
mod test;
