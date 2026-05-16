#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, String, Symbol, Vec,
};

// ── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const GRANT_CNT: Symbol = symbol_short!("GRNT_CNT");

fn grant_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("GRANT"), id)
}

fn application_key(grant_id: u64, applicant: &Address) -> (Symbol, u64, Address) {
    (symbol_short!("APP"), grant_id, applicant.clone())
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum GrantStatus {
    Open,
    Active,
    Completed,
    Revoked,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum MilestoneStatus {
    Pending,
    Submitted,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum ApplicationStatus {
    Applied,
    Accepted,
    Rejected,
}

#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub index: u32,
    pub description: String,
    pub amount: i128,
    pub evidence: String,
    pub status: MilestoneStatus,
}

#[contracttype]
#[derive(Clone)]
pub struct Grant {
    pub id: u64,
    pub grantor: Address,
    pub grantee: Address,
    pub token: Address,
    pub total_amount: i128,
    pub disbursed_amount: i128,
    pub title: String,
    pub description: String,
    pub milestones: Vec<Milestone>,
    pub status: GrantStatus,
}

#[contracttype]
#[derive(Clone)]
pub struct GrantApplication {
    pub grant_id: u64,
    pub applicant: Address,
    pub proposal: String,
    pub status: ApplicationStatus,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct GrantsContract;

#[contractimpl]
impl GrantsContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&GRANT_CNT, &0u64);
    }

    /// Grantor creates a grant and locks the full amount in the contract.
    pub fn create_grant(
        env: Env,
        grantor: Address,
        grantee: Address,
        token: Address,
        title: String,
        description: String,
        milestones: Vec<Milestone>,
    ) -> u64 {
        grantor.require_auth();
        assert!(!milestones.is_empty(), "need at least one milestone");

        let total: i128 = milestones.iter().map(|m| m.amount).sum();
        assert!(total > 0, "total must be positive");

        // Lock full grant amount upfront
        token::Client::new(&env, &token).transfer(
            &grantor,
            &env.current_contract_address(),
            &total,
        );

        let id: u64 = env.storage().instance().get(&GRANT_CNT).unwrap_or(0);
        let grant = Grant {
            id,
            grantor: grantor.clone(),
            grantee: grantee.clone(),
            token,
            total_amount: total,
            disbursed_amount: 0,
            title,
            description,
            milestones,
            status: GrantStatus::Active,
        };

        env.storage().persistent().set(&grant_key(id), &grant);
        env.storage().instance().set(&GRANT_CNT, &(id + 1));

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("created")),
            (id, grantor, grantee, total),
        );
        id
    }

    /// Grantor opens a grant for applications without locking funds yet.
    pub fn open_grant(
        env: Env,
        grantor: Address,
        token: Address,
        total_amount: i128,
        title: String,
        description: String,
        milestones: Vec<Milestone>,
    ) -> u64 {
        grantor.require_auth();
        assert!(total_amount > 0, "total must be positive");
        assert!(!milestones.is_empty(), "need at least one milestone");

        let milestone_total: i128 = milestones.iter().map(|m| m.amount).sum();
        assert!(
            milestone_total == total_amount,
            "milestone total mismatch"
        );

        let id: u64 = env.storage().instance().get(&GRANT_CNT).unwrap_or(0);
        let grant = Grant {
            id,
            grantor: grantor.clone(),
            grantee: grantor.clone(),
            token,
            total_amount,
            disbursed_amount: 0,
            title,
            description,
            milestones,
            status: GrantStatus::Open,
        };

        env.storage().persistent().set(&grant_key(id), &grant);
        env.storage().instance().set(&GRANT_CNT, &(id + 1));

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("opened")),
            (id, grantor, total_amount),
        );
        id
    }

    /// Applicant submits a proposal for an open grant.
    pub fn apply_for_grant(env: Env, applicant: Address, grant_id: u64, proposal: String) {
        applicant.require_auth();

        let g: Grant = env
            .storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found");
        assert!(g.status == GrantStatus::Open, "grant not open");

        let key = application_key(grant_id, &applicant);
        assert!(
            !env.storage().persistent().has(&key),
            "application exists"
        );

        let application = GrantApplication {
            grant_id,
            applicant: applicant.clone(),
            proposal,
            status: ApplicationStatus::Applied,
        };
        env.storage().persistent().set(&key, &application);

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("applied")),
            (grant_id, applicant),
        );
    }

    /// Grantor accepts an applicant, locks grant funds, and activates the grant.
    pub fn accept_application(env: Env, grant_id: u64, applicant: Address) {
        let mut g: Grant = env
            .storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found");
        g.grantor.require_auth();
        assert!(g.status == GrantStatus::Open, "grant not open");

        let key = application_key(grant_id, &applicant);
        let mut application: GrantApplication = env
            .storage()
            .persistent()
            .get(&key)
            .expect("application not found");
        assert!(
            application.status == ApplicationStatus::Applied,
            "application not active"
        );

        token::Client::new(&env, &g.token).transfer(
            &g.grantor,
            &env.current_contract_address(),
            &g.total_amount,
        );

        application.status = ApplicationStatus::Accepted;
        env.storage().persistent().set(&key, &application);

        g.grantee = applicant.clone();
        g.status = GrantStatus::Active;
        env.storage().persistent().set(&grant_key(grant_id), &g);

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("accepted")),
            (grant_id, applicant, g.total_amount),
        );
    }

    /// Grantee submits evidence for a specific milestone.
    pub fn submit_milestone(env: Env, grant_id: u64, milestone_index: u32, evidence: String) {
        let mut g: Grant = env
            .storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found");
        g.grantee.require_auth();
        assert!(g.status == GrantStatus::Active, "grant not active");

        let idx = milestone_index as usize;
        assert!(idx < g.milestones.len() as usize, "invalid milestone");
        let mut m = g.milestones.get(milestone_index).unwrap();
        assert!(
            m.status == MilestoneStatus::Pending || m.status == MilestoneStatus::Rejected,
            "cannot submit"
        );

        m.evidence = evidence;
        m.status = MilestoneStatus::Submitted;
        g.milestones.set(milestone_index, m);
        env.storage().persistent().set(&grant_key(grant_id), &g);

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("submitted")),
            (grant_id, milestone_index),
        );
    }

    /// Grantor approves a submitted milestone and releases its funds to grantee.
    pub fn approve_milestone(env: Env, grant_id: u64, milestone_index: u32) {
        let mut g: Grant = env
            .storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found");
        g.grantor.require_auth();
        assert!(g.status == GrantStatus::Active, "grant not active");

        let idx = milestone_index as usize;
        assert!(idx < g.milestones.len() as usize, "invalid milestone");
        let mut m = g.milestones.get(milestone_index).unwrap();
        assert!(m.status == MilestoneStatus::Submitted, "not submitted");

        let payout = m.amount;
        m.status = MilestoneStatus::Approved;
        g.milestones.set(milestone_index, m);
        g.disbursed_amount += payout;

        token::Client::new(&env, &g.token).transfer(
            &env.current_contract_address(),
            &g.grantee,
            &payout,
        );

        // Auto-complete when all milestones approved
        let all_approved = g
            .milestones
            .iter()
            .all(|m| m.status == MilestoneStatus::Approved);
        if all_approved {
            g.status = GrantStatus::Completed;
            env.events().publish(
                (symbol_short!("grant"), symbol_short!("completed")),
                grant_id,
            );
        }

        env.storage().persistent().set(&grant_key(grant_id), &g);

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("approved")),
            (grant_id, milestone_index, payout),
        );
    }

    /// Grantor rejects a submitted milestone; resets to Pending with evidence cleared.
    pub fn reject_milestone(env: Env, grant_id: u64, milestone_index: u32) {
        let mut g: Grant = env
            .storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found");
        g.grantor.require_auth();
        assert!(g.status == GrantStatus::Active, "grant not active");

        let idx = milestone_index as usize;
        assert!(idx < g.milestones.len() as usize, "invalid milestone");
        let mut m = g.milestones.get(milestone_index).unwrap();
        assert!(m.status == MilestoneStatus::Submitted, "not submitted");

        m.evidence = String::from_str(&env, "");
        m.status = MilestoneStatus::Pending;
        g.milestones.set(milestone_index, m);
        env.storage().persistent().set(&grant_key(grant_id), &g);

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("rejected")),
            (grant_id, milestone_index),
        );
    }

    /// Grantor revokes an active grant and reclaims undisbursed funds.
    pub fn revoke_grant(env: Env, grant_id: u64) {
        let mut g: Grant = env
            .storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found");
        g.grantor.require_auth();
        assert!(g.status == GrantStatus::Active, "grant not active");

        let remaining = g.total_amount - g.disbursed_amount;
        g.status = GrantStatus::Revoked;
        env.storage().persistent().set(&grant_key(grant_id), &g);

        if remaining > 0 {
            token::Client::new(&env, &g.token).transfer(
                &env.current_contract_address(),
                &g.grantor,
                &remaining,
            );
        }

        env.events().publish(
            (symbol_short!("grant"), symbol_short!("revoked")),
            (grant_id, remaining),
        );
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn get_grant(env: Env, grant_id: u64) -> Grant {
        env.storage()
            .persistent()
            .get(&grant_key(grant_id))
            .expect("grant not found")
    }

    pub fn get_application(env: Env, grant_id: u64, applicant: Address) -> GrantApplication {
        env.storage()
            .persistent()
            .get(&application_key(grant_id, &applicant))
            .expect("application not found")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::Address as _,
        token::{Client as TokenClient, StellarAssetClient},
        Env, String, Vec,
    };

    fn setup() -> (Env, GrantsContractClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, GrantsContract);
        let client = GrantsContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        let grantor = Address::generate(&env);
        let grantee = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin, grantor, grantee)
    }

    fn make_milestones(env: &Env) -> Vec<Milestone> {
        let mut v = Vec::new(env);
        v.push_back(Milestone {
            index: 0,
            description: String::from_str(env, "Phase 1"),
            amount: 300,
            evidence: String::from_str(env, ""),
            status: MilestoneStatus::Pending,
        });
        v.push_back(Milestone {
            index: 1,
            description: String::from_str(env, "Phase 2"),
            amount: 700,
            evidence: String::from_str(env, ""),
            status: MilestoneStatus::Pending,
        });
        v
    }

    #[test]
    fn full_grant_lifecycle() {
        let (env, client, admin, grantor, grantee) = setup();
        let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
        StellarAssetClient::new(&env, &token_addr).mint(&grantor, &1_000);

        let milestones = make_milestones(&env);
        let grant_id = client.create_grant(
            &grantor,
            &grantee,
            &token_addr,
            &String::from_str(&env, "Build"),
            &String::from_str(&env, "Desc"),
            &milestones,
        );

        client.submit_milestone(&grant_id, &0u32, &String::from_str(&env, "proof1"));
        client.approve_milestone(&grant_id, &0u32);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&grantee), 300);

        client.submit_milestone(&grant_id, &1u32, &String::from_str(&env, "proof2"));
        client.approve_milestone(&grant_id, &1u32);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&grantee), 1_000);
        assert_eq!(client.get_grant(&grant_id).status, GrantStatus::Completed);
    }

    #[test]
    fn reject_then_resubmit() {
        let (env, client, admin, grantor, grantee) = setup();
        let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
        StellarAssetClient::new(&env, &token_addr).mint(&grantor, &300);

        let mut milestones = Vec::new(&env);
        milestones.push_back(Milestone {
            index: 0,
            description: String::from_str(&env, "M1"),
            amount: 300,
            evidence: String::from_str(&env, ""),
            status: MilestoneStatus::Pending,
        });
        let id = client.create_grant(
            &grantor,
            &grantee,
            &token_addr,
            &String::from_str(&env, "G"),
            &String::from_str(&env, "D"),
            &milestones,
        );

        client.submit_milestone(&id, &0u32, &String::from_str(&env, "bad proof"));
        client.reject_milestone(&id, &0u32);
        let m = client.get_grant(&id).milestones.get(0).unwrap();
        assert_eq!(m.status, MilestoneStatus::Pending);

        client.submit_milestone(&id, &0u32, &String::from_str(&env, "good proof"));
        client.approve_milestone(&id, &0u32);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&grantee), 300);
    }

    #[test]
    fn revoke_reclaims_undisbursed() {
        let (env, client, admin, grantor, grantee) = setup();
        let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
        StellarAssetClient::new(&env, &token_addr).mint(&grantor, &1_000);

        let milestones = make_milestones(&env);
        let id = client.create_grant(
            &grantor,
            &grantee,
            &token_addr,
            &String::from_str(&env, "G"),
            &String::from_str(&env, "D"),
            &milestones,
        );

        client.submit_milestone(&id, &0u32, &String::from_str(&env, "proof"));
        client.approve_milestone(&id, &0u32); // 300 disbursed
        client.revoke_grant(&id);

        // grantor gets back 700
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&grantor), 700);
        assert_eq!(client.get_grant(&id).status, GrantStatus::Revoked);
    }

    #[test]
    fn open_grant_accepts_application_and_activates_lifecycle() {
        let (env, client, admin, grantor, grantee) = setup();
        let second_applicant = Address::generate(&env);
        let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
        StellarAssetClient::new(&env, &token_addr).mint(&grantor, &1_000);

        let milestones = make_milestones(&env);
        let grant_id = client.open_grant(
            &grantor,
            &token_addr,
            &1_000i128,
            &String::from_str(&env, "Open grant"),
            &String::from_str(&env, "Apply to build"),
            &milestones,
        );

        assert_eq!(client.get_grant(&grant_id).status, GrantStatus::Open);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&grantor), 1_000);

        client.apply_for_grant(&grantee, &grant_id, &String::from_str(&env, "Proposal A"));
        client.apply_for_grant(
            &second_applicant,
            &grant_id,
            &String::from_str(&env, "Proposal B"),
        );

        let application = client.get_application(&grant_id, &second_applicant);
        assert_eq!(application.grant_id, grant_id);
        assert_eq!(application.applicant, second_applicant);
        assert_eq!(application.status, ApplicationStatus::Applied);

        client.accept_application(&grant_id, &application.applicant);
        let accepted = client.get_application(&grant_id, &application.applicant);
        assert_eq!(accepted.status, ApplicationStatus::Accepted);

        let grant = client.get_grant(&grant_id);
        assert_eq!(grant.status, GrantStatus::Active);
        assert_eq!(grant.grantee, application.applicant);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&grantor), 0);

        client.submit_milestone(&grant_id, &0u32, &String::from_str(&env, "proof1"));
        client.approve_milestone(&grant_id, &0u32);
        assert_eq!(
            TokenClient::new(&env, &token_addr).balance(&accepted.applicant),
            300
        );
    }
}
