#![no_std]
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, token, Address, BytesN, Env, String,
};

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
    pub juror: Option<Address>,
    pub token: Address,
    pub amount: i128,
    pub status: JobStatus,
    pub start_time: u64,
    pub end_time: u64,
    pub deadline: u64,
    pub dispute_reason: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Job(u64),
    JobCounter,
    RegistryContract,
    Admin,
    IsPaused,
    PlatformFee,
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
pub struct DisputeRaised {
    pub id: u64,
    pub raised_by: Address,
}

#[contractevent]
pub struct DisputeResolved {
    pub id: u64,
    pub finder_share: i128,
    pub artisan_share: i128,
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

#[contractevent]
pub struct AdminTransferred {
    #[topic]
    pub new_admin: Address,
}

#[contractevent]
pub struct PauseStateChanged {
    pub paused: bool,
}

#[contractevent]
pub struct EmergencyWithdraw {
    pub token: Address,
    pub amount: i128,
    pub to: Address,
}

#[contractevent]
pub struct ContractUpgraded {
    pub hash: BytesN<32>,
}

#[contractevent]
pub struct FeeUpdated {
    pub new_fee_bps: u32,
}

#[contractevent]
pub struct JurorAssigned {
    pub id: u64,
    pub juror: Address,
}

#[contract]
pub struct MarketContract;


pub fn is_paused(env: &Env) -> bool {
    let paused = env
        .storage()
        .instance()
        .get(&DataKey::IsPaused)
        .expect("Missing storage variable");
    env.storage().instance().extend_ttl(100_000, 500_000);
    paused
}

#[contractimpl]
impl MarketContract {
    pub fn initialize(env: Env, registry_contract: Address, admin: &Address) {
        if env.storage().instance().has(&DataKey::RegistryContract) {
            panic!("Registry already initialized");
        }
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Admin already initialized");
        }
        env.storage()
            .instance()
            .set(&DataKey::RegistryContract, &registry_contract);
        env.storage().instance().set(&DataKey::Admin, admin);
        env.storage().instance().set(&DataKey::IsPaused, &false);
    }

    pub fn create_job(env: Env, finder: Address, token: Address, amount: i128) -> u64 {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage().instance().extend_ttl(100_000, 500_000);

        let job = Job {
            id,
            finder,
            artisan: None,
            juror: None,
            token,
            amount,
            status: JobStatus::Open,
            start_time: 0,
            end_time: 0,
            deadline: 0,
            dispute_reason: None,
        };
        env.storage().persistent().set(&DataKey::Job(id), &job);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(id), 100_000, 500_000);

        JobCreated { id, amount }.publish(&env);

        id
    }

    pub fn assign_artisan(env: Env, finder: Address, job_id: u64, artisan: Address) {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        JobAssigned {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn apply_for_job(env: Env, artisan: Address, job_id: u64) {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

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
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        JobStarted {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn cancel_job(env: Env, finder: Address, job_id: u64) {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        JobCancelled { id: job_id }.publish(&env);
    }

    pub fn complete_job(env: Env, artisan: Address, job_id: u64) {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        JobCompleted {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn confirm_delivery(env: Env, finder: Address, job_id: u64) {
        assert!(!is_paused(&env), "Contract Paused");
        finder.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.finder != finder {
            panic!("Not job owner");
        }

        if job.status != JobStatus::PendingReview {
            panic!("Job is not pending review");
        }

        let artisan = job.artisan.clone().expect("Job has no assigned artisan");

        let fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .unwrap_or(100);
        let fee = (job.amount * (fee_bps as i128)) / 10000;
        let payout = job.amount - fee;

        let token_client = token::TokenClient::new(&env, &job.token);
        let contract = env.current_contract_address();
        token_client.transfer(&contract, &artisan, &payout);
        token_client.transfer(&contract, &admin, &fee);

        job.status = JobStatus::Completed;
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        FundsReleased {
            id: job_id,
            artisan,
            amount: payout,
        }
        .publish(&env);
    }

    pub fn raise_dispute(env: Env, caller: Address, job_id: u64) {
        caller.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        if job.finder != caller && job.artisan.as_ref() != Some(&caller) {
            panic!("Only the finder or assigned artisan can raise a dispute");
        }

        if job.status != JobStatus::InProgress && job.status != JobStatus::PendingReview {
            panic!("Job cannot be disputed in its current status");
        }

        job.status = JobStatus::Disputed;
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        DisputeRaised {
            id: job_id,
            raised_by: caller,
        }
        .publish(&env);
    }

    pub fn auto_release_funds(env: Env, artisan: Address, job_id: u64) {
        assert!(!is_paused(&env), "Contract Paused");
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

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");

        let fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .unwrap_or(100);
        let fee = (job.amount * (fee_bps as i128)) / 10000;
        let payout = job.amount - fee;

        let token_client = token::TokenClient::new(&env, &job.token);
        let contract = env.current_contract_address();
        token_client.transfer(&contract, &artisan, &payout);
        if fee > 0 {
            token_client.transfer(&contract, &admin, &fee);
        }

        job.status = JobStatus::Completed;
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        FundsReleased {
            id: job_id,
            artisan,
            amount: payout,
        }
        .publish(&env);
    }

    pub fn extend_deadline(env: Env, finder: Address, job_id: u64, extra_time: u64) {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        DeadlineExtended {
            id: job_id,
            extra_time,
            new_deadline: job.deadline,
        }
        .publish(&env);
    }

    pub fn increase_budget(env: Env, finder: Address, job_id: u64, added_amount: i128) {
        assert!(!is_paused(&env), "Contract Paused");
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        BudgetIncreased {
            id: job_id,
            added_amount,
            new_amount: job.amount,
        }
        .publish(&env);
    }

    pub fn transfer_admin(env: Env, old_admin: Address, new_admin: Address) {
        assert!(!is_paused(&env), "Contract Paused");
        old_admin.require_auth();

        let current_admin = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(old_admin == current_admin, "Unauthorized caller");

        env.storage().instance().set(&DataKey::Admin, &new_admin);

        AdminTransferred { new_admin }.publish(&env);
    }

    pub fn toggle_contract_pause(env: Env, admin: Address) {
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(admin == current_admin, "Unauthorized caller");

        let mut paused = env
            .storage()
            .instance()
            .get(&DataKey::IsPaused)
            .expect("Pause state not set");

        if paused {
            env.storage().instance().set(&DataKey::IsPaused, &false);
            paused = false;
        } else {
            env.storage().instance().set(&DataKey::IsPaused, &true);
            paused = true;
        }

        PauseStateChanged { paused }.publish(&env);
    }

    pub fn emergency_withdraw(env: Env, admin: Address, token: Address, amount: i128, to: Address) {
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(admin == current_admin, "Unauthorized caller");

        assert!(is_paused(&env), "Contract is not paused");

        let token_client = token::TokenClient::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &to, &amount);

        EmergencyWithdraw { token, amount, to }.publish(&env);
    }

    pub fn upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(admin == current_admin, "Unauthorized caller");

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());

        ContractUpgraded {
            hash: new_wasm_hash,
        }
        .publish(&env);
    }

    pub fn set_platform_fee(env: Env, admin: Address, fee_bps: u32) {
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(admin == current_admin, "Unauthorized caller");

        assert!(fee_bps <= 1000, "Fee exceeds maximum allowed (1000 bps)");

        env.storage()
            .instance()
            .set(&DataKey::PlatformFee, &fee_bps);

        FeeUpdated {
            new_fee_bps: fee_bps,
        }
        .publish(&env);
    }

    pub fn assign_juror(env: Env, admin: Address, job_id: u64, juror: Address) {
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(admin == current_admin, "Unauthorized caller");

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

        assert!(job.status == JobStatus::Disputed, "Job is not disputed");

        let registry_client = registry::Client::new(&env, &registry_contract);
        let profile = registry_client.get_profile(&juror);

        assert!(profile.role == 1, "User is not a Curator");

        job.juror = Some(juror.clone());
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        JurorAssigned { id: job_id, juror }.publish(&env);
    }

    pub fn resolve_dispute(
        env: Env,
        juror: Address,
        job_id: u64,
        finder_share: i128,
        artisan_share: i128,
    ) {
        juror.require_auth();

        let mut job: Job = env
            .storage()
            .persistent()
            .get(&DataKey::Job(job_id))
            .expect("Job not found");

        assert!(job.status == JobStatus::Disputed, "Job is not disputed");
        assert!(job.juror == Some(juror.clone()), "Not assigned juror");

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");

        let fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .unwrap_or(100);
        let fee = (job.amount * (fee_bps as i128)) / 10000;
        assert!(
            finder_share + artisan_share + fee == job.amount,
            "Invalid shares"
        );

        let token_client = token::TokenClient::new(&env, &job.token);
        let contract = env.current_contract_address();

        if finder_share > 0 {
            token_client.transfer(&contract, &job.finder, &finder_share);
        }

        if artisan_share > 0 {
            let artisan = job.artisan.clone().expect("Job has no assigned artisan");
            token_client.transfer(&contract, &artisan, &artisan_share);
        }

        if fee > 0 {
            token_client.transfer(&contract, &admin, &fee);
        }

        job.status = JobStatus::Completed;
        env.storage().persistent().set(&DataKey::Job(job_id), &job);

        DisputeResolved {
            id: job_id,
            finder_share,
            artisan_share,
        }
        .publish(&env);
    }
}

#[cfg(test)]
mod test;
