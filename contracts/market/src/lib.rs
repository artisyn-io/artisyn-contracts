#![no_std]
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, token, Address, BytesN, Env, String, Vec,
};

pub const ASSIGNMENT_TIMEOUT_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const DEFAULT_DEADLINE_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MAX_SINGLE_EXTENSION_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MAX_CUMULATIVE_EXTENSION_SECONDS: u64 = 90 * 24 * 60 * 60;

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
    pub total_extended: u64,
    pub dispute_reason: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobApplicationRecord {
    pub job_id: u64,
    pub artisan: Address,
    pub applied_at: u64,
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
    AssignmentTime(u64),
    Application(u64, Address),
    JobApplicants(u64),
    CollectedFees(Address),
    DefaultDeadline,
    MaxSingleExtension,
    MaxCumulativeExtension,
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
pub struct AssignmentTimedOut {
    pub id: u64,
    pub artisan: Address,
}

#[contractevent]
pub struct JobReassigned {
    pub id: u64,
    pub previous_artisan: Address,
    pub new_artisan: Address,
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
    pub reason: String,
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

#[contractevent]
pub struct FeeCollected {
    pub id: u64,
    pub token: Address,
    pub amount: i128,
}

#[contractevent]
pub struct DeadlinePolicyUpdated {
    pub default_deadline: u64,
    pub max_single_extension: u64,
    pub max_cumulative_extension: u64,
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

fn record_fee_collected(env: &Env, job_id: u64, token: &Address, amount: i128) {
    let key = DataKey::CollectedFees(token.clone());
    let total: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(total + amount));
    env.storage()
        .persistent()
        .extend_ttl(&key, 100_000, 500_000);

    FeeCollected {
        id: job_id,
        token: token.clone(),
        amount,
    }
    .publish(env);
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
        env.storage()
            .instance()
            .set(&DataKey::DefaultDeadline, &DEFAULT_DEADLINE_SECONDS);
        env.storage()
            .instance()
            .set(&DataKey::MaxSingleExtension, &MAX_SINGLE_EXTENSION_SECONDS);
        env.storage().instance().set(
            &DataKey::MaxCumulativeExtension,
            &MAX_CUMULATIVE_EXTENSION_SECONDS,
        );
    }

    pub fn create_job(
        env: Env,
        finder: Address,
        token: Address,
        amount: i128,
        initial_deadline: u64,
    ) -> u64 {
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

        let deadline = if initial_deadline == 0 {
            DEFAULT_DEADLINE_SECONDS
        } else {
            initial_deadline
        };

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
            deadline,
            total_extended: 0,
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
            .set(&DataKey::AssignmentTime(job_id), &env.ledger().timestamp());
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::AssignmentTime(job_id), 100_000, 500_000);

        JobAssigned {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    pub fn reopen_timed_out_assignment(env: Env, finder: Address, job_id: u64) {
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
        if job.status != JobStatus::Assigned {
            panic!("Job is not assigned");
        }

        let assigned_at: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::AssignmentTime(job_id))
            .expect("Assignment time not found");
        let timeout_at = assigned_at
            .checked_add(ASSIGNMENT_TIMEOUT_SECONDS)
            .expect("Assignment timeout overflow");
        if env.ledger().timestamp() < timeout_at {
            panic!("Assignment has not timed out");
        }

        let artisan = job.artisan.take().expect("Job has no assigned artisan");
        job.status = JobStatus::Open;

        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .remove(&DataKey::AssignmentTime(job_id));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        AssignmentTimedOut {
            id: job_id,
            artisan,
        }
        .publish(&env);
    }

    /// Reassigns a stalled assignment without moving or refunding escrow.
    ///
    /// A finder may reassign only while the job is still `Assigned` and the
    /// current artisan has not started it within the assignment timeout.
    pub fn reassign_artisan(env: Env, finder: Address, job_id: u64, new_artisan: Address) {
        assert!(!is_paused(&env), "Contract Paused");
        finder.require_auth();

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

        if job.finder != finder {
            panic!("Not job owner");
        }
        if job.status != JobStatus::Assigned {
            panic!("Job is not assigned");
        }

        let previous_artisan = job.artisan.clone().expect("Job has no assigned artisan");
        if previous_artisan == new_artisan {
            panic!("Artisan is already assigned");
        }

        let assigned_at: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::AssignmentTime(job_id))
            .expect("Assignment time not found");
        let timeout_at = assigned_at
            .checked_add(ASSIGNMENT_TIMEOUT_SECONDS)
            .expect("Assignment timeout overflow");
        if env.ledger().timestamp() < timeout_at {
            panic!("Assignment has not timed out");
        }

        let registry_client = registry::Client::new(&env, &registry_contract);
        let profile = registry_client.get_profile(&new_artisan);
        if profile.role != 3 {
            panic!("User is not a verified Artisan");
        }
        if profile.is_blacklisted {
            panic!("User is blacklisted");
        }

        job.artisan = Some(new_artisan.clone());
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .set(&DataKey::AssignmentTime(job_id), &env.ledger().timestamp());
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::AssignmentTime(job_id), 100_000, 500_000);

        JobReassigned {
            id: job_id,
            previous_artisan,
            new_artisan,
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

        let app_key = DataKey::Application(job_id, artisan.clone());
        if env.storage().persistent().has(&app_key) {
            panic!("Duplicate application");
        }

        let record = JobApplicationRecord {
            job_id,
            artisan: artisan.clone(),
            applied_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&app_key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&app_key, 100_000, 500_000);

        let applicants_key = DataKey::JobApplicants(job_id);
        let mut applicants: Vec<Address> = env
            .storage()
            .persistent()
            .get(&applicants_key)
            .unwrap_or_else(|| Vec::new(&env));
        applicants.push_back(artisan.clone());
        env.storage().persistent().set(&applicants_key, &applicants);
        env.storage()
            .persistent()
            .extend_ttl(&applicants_key, 100_000, 500_000);

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
            .remove(&DataKey::AssignmentTime(job_id));
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
        if fee > 0 {
            token_client.transfer(&contract, &admin, &fee);
            record_fee_collected(&env, job_id, &job.token, fee);
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

    pub fn raise_dispute(env: Env, caller: Address, job_id: u64, reason: String) {
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
        job.dispute_reason = Some(reason.clone());
        env.storage().persistent().set(&DataKey::Job(job_id), &job);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Job(job_id), 100_000, 500_000);

        DisputeRaised {
            id: job_id,
            raised_by: caller,
            reason,
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
            record_fee_collected(&env, job_id, &job.token, fee);
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

        assert!(extra_time > 0, "Extension must be greater than zero");

        let max_single: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSingleExtension)
            .unwrap_or(MAX_SINGLE_EXTENSION_SECONDS);
        let max_cumulative: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxCumulativeExtension)
            .unwrap_or(MAX_CUMULATIVE_EXTENSION_SECONDS);

        assert!(
            extra_time <= max_single,
            "Extension exceeds maximum single extension"
        );
        let new_total = job
            .total_extended
            .checked_add(extra_time)
            .expect("Total extension overflow");
        assert!(
            new_total <= max_cumulative,
            "Cumulative extension exceeds cap"
        );

        let new_deadline = job
            .deadline
            .checked_add(extra_time)
            .expect("Deadline overflow");

        job.deadline = new_deadline;
        job.total_extended = new_total;

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

    pub fn set_deadline_policy(
        env: Env,
        admin: Address,
        default_deadline: u64,
        max_single_extension: u64,
        max_cumulative_extension: u64,
    ) {
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not set");
        assert!(admin == current_admin, "Unauthorized caller");

        assert!(
            default_deadline > 0,
            "Default deadline must be greater than zero"
        );
        assert!(
            max_single_extension > 0,
            "Max single extension must be greater than zero"
        );
        assert!(
            max_cumulative_extension > 0,
            "Max cumulative extension must be greater than zero"
        );
        assert!(
            max_single_extension <= max_cumulative_extension,
            "Max single extension must not exceed cumulative cap"
        );

        env.storage()
            .instance()
            .set(&DataKey::DefaultDeadline, &default_deadline);
        env.storage()
            .instance()
            .set(&DataKey::MaxSingleExtension, &max_single_extension);
        env.storage()
            .instance()
            .set(&DataKey::MaxCumulativeExtension, &max_cumulative_extension);

        DeadlinePolicyUpdated {
            default_deadline,
            max_single_extension,
            max_cumulative_extension,
        }
        .publish(&env);
    }

    pub fn get_deadline_policy(env: Env) -> (u64, u64, u64) {
        let default_deadline: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DefaultDeadline)
            .unwrap_or(DEFAULT_DEADLINE_SECONDS);
        let max_single_extension: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSingleExtension)
            .unwrap_or(MAX_SINGLE_EXTENSION_SECONDS);
        let max_cumulative_extension: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxCumulativeExtension)
            .unwrap_or(MAX_CUMULATIVE_EXTENSION_SECONDS);
        (
            default_deadline,
            max_single_extension,
            max_cumulative_extension,
        )
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
            record_fee_collected(&env, job_id, &job.token, fee);
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

    pub fn get_collected_fees(env: Env, token: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::CollectedFees(token))
            .unwrap_or(0)
    }

    pub fn get_job_applicants(env: Env, job_id: u64) -> Vec<Address> {
        let key = DataKey::JobApplicants(job_id);
        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, 100_000, 500_000);
            env.storage().persistent().get(&key).unwrap()
        } else {
            Vec::new(&env)
        }
    }

    pub fn get_application(
        env: Env,
        job_id: u64,
        artisan: Address,
    ) -> Option<JobApplicationRecord> {
        let key = DataKey::Application(job_id, artisan);
        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, 100_000, 500_000);
            env.storage().persistent().get(&key)
        } else {
            None
        }
    }

    pub fn has_applied(env: Env, job_id: u64, artisan: Address) -> bool {
        let key = DataKey::Application(job_id, artisan);
        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, 100_000, 500_000);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod test;
