#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, String, Symbol, Vec,
};

// ── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const ROUND_CNT: Symbol = symbol_short!("RND_CNT");

fn round_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("ROUND"), id)
}

fn project_key(round_id: u64, project_id: u64) -> (Symbol, u64, u64) {
    (symbol_short!("PROJ"), round_id, project_id)
}

fn contrib_key(round_id: u64, project_id: u64, contributor: &Address) -> (Symbol, u64, u64, Address) {
    (symbol_short!("CONTRIB"), round_id, project_id, contributor.clone())
}

fn proj_cnt_key(round_id: u64) -> (Symbol, u64) {
    (symbol_short!("PROJ_CNT"), round_id)
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum RoundStatus {
    Active,
    Finalized,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct Round {
    pub id: u64,
    pub admin: Address,
    pub token: Address,
    pub matching_pool: i128,
    pub title: String,
    pub description: String,
    pub start_ledger: u32,
    pub end_ledger: u32,
    pub status: RoundStatus,
}

#[contracttype]
#[derive(Clone)]
pub struct Project {
    pub id: u64,
    pub round_id: u64,
    pub owner: Address,
    pub title: String,
    pub description: String,
    pub total_contributions: i128,
    pub contributor_count: u32,
    pub matching_amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct Contribution {
    pub round_id: u64,
    pub project_id: u64,
    pub contributor: Address,
    pub amount: i128,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Integer square root via Newton's method.
fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct QuadraticContract;

#[contractimpl]
impl QuadraticContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&ROUND_CNT, &0u64);
    }

    /// Admin creates a round and locks the matching pool in the contract.
    pub fn create_round(
        env: Env,
        admin: Address,
        token: Address,
        matching_pool: i128,
        title: String,
        description: String,
        start_ledger: u32,
        end_ledger: u32,
    ) -> u64 {
        admin.require_auth();
        assert!(matching_pool > 0, "matching pool must be positive");
        assert!(end_ledger > start_ledger, "invalid ledger range");

        token::Client::new(&env, &token).transfer(
            &admin,
            &env.current_contract_address(),
            &matching_pool,
        );

        let id: u64 = env.storage().instance().get(&ROUND_CNT).unwrap_or(0);
        let round = Round {
            id,
            admin: admin.clone(),
            token,
            matching_pool,
            title,
            description,
            start_ledger,
            end_ledger,
            status: RoundStatus::Active,
        };

        env.storage().persistent().set(&round_key(id), &round);
        env.storage().instance().set(&ROUND_CNT, &(id + 1));
        env.storage().persistent().set(&proj_cnt_key(id), &0u64);

        env.events().publish(
            (symbol_short!("round"), symbol_short!("created")),
            (id, admin, matching_pool),
        );
        id
    }

    /// Register a project in an active round.
    pub fn register_project(
        env: Env,
        round_id: u64,
        owner: Address,
        title: String,
        description: String,
    ) -> u64 {
        owner.require_auth();
        let round: Round = env
            .storage()
            .persistent()
            .get(&round_key(round_id))
            .expect("round not found");
        assert!(round.status == RoundStatus::Active, "round not active");

        let proj_id: u64 = env
            .storage()
            .persistent()
            .get(&proj_cnt_key(round_id))
            .unwrap_or(0);

        let project = Project {
            id: proj_id,
            round_id,
            owner: owner.clone(),
            title,
            description,
            total_contributions: 0,
            contributor_count: 0,
            matching_amount: 0,
        };

        env.storage()
            .persistent()
            .set(&project_key(round_id, proj_id), &project);
        env.storage()
            .persistent()
            .set(&proj_cnt_key(round_id), &(proj_id + 1));

        env.events().publish(
            (symbol_short!("round"), symbol_short!("proj_reg")),
            (round_id, proj_id, owner),
        );
        proj_id
    }

    /// Contribute to a project. First contribution per address increments contributor_count.
    pub fn contribute(env: Env, contributor: Address, round_id: u64, project_id: u64, amount: i128) {
        contributor.require_auth();
        assert!(amount > 0, "amount must be positive");

        let round: Round = env
            .storage()
            .persistent()
            .get(&round_key(round_id))
            .expect("round not found");
        assert!(round.status == RoundStatus::Active, "round not active");
        assert!(
            env.ledger().sequence() >= round.start_ledger
                && env.ledger().sequence() <= round.end_ledger,
            "outside round window"
        );

        let mut project: Project = env
            .storage()
            .persistent()
            .get(&project_key(round_id, project_id))
            .expect("project not found");

        token::Client::new(&env, &round.token).transfer(
            &contributor,
            &env.current_contract_address(),
            &amount,
        );

        let ck = contrib_key(round_id, project_id, &contributor);
        let mut rec: Contribution = env
            .storage()
            .persistent()
            .get(&ck)
            .unwrap_or(Contribution {
                round_id,
                project_id,
                contributor: contributor.clone(),
                amount: 0,
            });

        // First contribution → increment unique contributor count
        if rec.amount == 0 {
            project.contributor_count += 1;
        }
        rec.amount += amount;
        project.total_contributions += amount;

        env.storage().persistent().set(&ck, &rec);
        env.storage()
            .persistent()
            .set(&project_key(round_id, project_id), &project);

        env.events().publish(
            (symbol_short!("round"), symbol_short!("contrib")),
            (round_id, project_id, contributor, amount),
        );
    }

    /// Finalize the round: compute quadratic matching and pay out all projects.
    pub fn finalize_round(env: Env, round_id: u64, project_ids: Vec<u64>) {
        let mut round: Round = env
            .storage()
            .persistent()
            .get(&round_key(round_id))
            .expect("round not found");
        round.admin.require_auth();
        assert!(round.status == RoundStatus::Active, "round not active");

        // Compute sqrt(contributor_count) per project and total
        let mut sqrts: Vec<i128> = Vec::new(&env);
        let mut sqrt_sum: i128 = 0;
        for pid in project_ids.iter() {
            let p: Project = env
                .storage()
                .persistent()
                .get(&project_key(round_id, pid))
                .expect("project not found");
            let s = isqrt(p.contributor_count as i128);
            sqrts.push_back(s);
            sqrt_sum += s;
        }

        // Distribute matching pool proportionally to sqrt sums, then pay out
        let tok = token::Client::new(&env, &round.token);
        for (i, pid) in project_ids.iter().enumerate() {
            let mut p: Project = env
                .storage()
                .persistent()
                .get(&project_key(round_id, pid))
                .expect("project not found");

            let matching = if sqrt_sum > 0 {
                round.matching_pool * sqrts.get(i as u32).unwrap_or(0) / sqrt_sum
            } else {
                0
            };
            p.matching_amount = matching;

            let total_payout = p.total_contributions + matching;
            if total_payout > 0 {
                tok.transfer(&env.current_contract_address(), &p.owner, &total_payout);
            }

            env.storage()
                .persistent()
                .set(&project_key(round_id, pid), &p);

            env.events().publish(
                (symbol_short!("round"), symbol_short!("payout")),
                (round_id, pid, total_payout),
            );
        }

        round.status = RoundStatus::Finalized;
        env.storage().persistent().set(&round_key(round_id), &round);

        env.events().publish(
            (symbol_short!("round"), symbol_short!("finalized")),
            round_id,
        );
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn get_round(env: Env, round_id: u64) -> Round {
        env.storage()
            .persistent()
            .get(&round_key(round_id))
            .expect("round not found")
    }

    pub fn get_project(env: Env, round_id: u64, project_id: u64) -> Project {
        env.storage()
            .persistent()
            .get(&project_key(round_id, project_id))
            .expect("project not found")
    }

    pub fn get_contribution(env: Env, round_id: u64, project_id: u64, contributor: Address) -> Contribution {
        env.storage()
            .persistent()
            .get(&contrib_key(round_id, project_id, &contributor))
            .expect("contribution not found")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger, MockAuth, MockAuthInvoke},
        token::{Client as TokenClient, StellarAssetClient},
        Env, IntoVal, String, Symbol, Val, Vec,
    };

    fn setup() -> (Env, QuadraticContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, QuadraticContract);
        let client = QuadraticContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    fn make_token(env: &Env, admin: &Address) -> Address {
        env.register_stellar_asset_contract_v2(admin.clone()).address()
    }

    fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
        StellarAssetClient::new(env, token).mint(to, &amount);
    }

    fn create_round(
        env: &Env,
        client: &QuadraticContractClient<'static>,
        admin: &Address,
        token_addr: &Address,
        matching_pool: i128,
        start_ledger: u32,
        end_ledger: u32,
    ) -> u64 {
        client.create_round(
            admin,
            token_addr,
            &matching_pool,
            &String::from_str(env, "Round"),
            &String::from_str(env, "Test round"),
            &start_ledger,
            &end_ledger,
        )
    }

    fn register_project(
        env: &Env,
        client: &QuadraticContractClient<'static>,
        round_id: u64,
        owner: &Address,
    ) -> u64 {
        client.register_project(
            &round_id,
            owner,
            &String::from_str(env, "Project"),
            &String::from_str(env, "Desc"),
        )
    }

    fn project_ids(env: &Env, ids: &[u64]) -> Vec<u64> {
        let mut v = Vec::new(env);
        for id in ids {
            v.push_back(*id);
        }
        v
    }

    fn assert_round_event(env: &Env, name: Symbol) {
        let expected_topics: Vec<Val> = (symbol_short!("round"), name).into_val(env);
        let mut found = false;

        for (_, topics, _) in env.events().all().iter() {
            if topics == expected_topics {
                found = true;
                break;
            }
        }

        assert!(found, "expected round event was not emitted");
    }

    #[test]
    fn isqrt_correctness() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(10), 3);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(1_000_000), 1_000);
    }

    #[test]
    fn full_round_lifecycle() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);

        let owner1 = Address::generate(&env);
        let owner2 = Address::generate(&env);
        let c1 = Address::generate(&env);
        let c2 = Address::generate(&env);
        let c3 = Address::generate(&env);

        mint(&env, &token_addr, &admin, 1_000);
        mint(&env, &token_addr, &c1, 500);
        mint(&env, &token_addr, &c2, 500);
        mint(&env, &token_addr, &c3, 500);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        assert_round_event(&env, symbol_short!("created"));

        let p1 = register_project(&env, &client, round_id, &owner1);
        let p2 = register_project(&env, &client, round_id, &owner2);
        assert_round_event(&env, symbol_short!("proj_reg"));

        // p1 gets 3 unique contributors, p2 gets 1
        client.contribute(&c1, &round_id, &p1, &100i128);
        client.contribute(&c2, &round_id, &p1, &100i128);
        client.contribute(&c3, &round_id, &p1, &100i128);
        client.contribute(&c1, &round_id, &p2, &100i128);
        assert_round_event(&env, symbol_short!("contrib"));

        let ids = project_ids(&env, &[p1, p2]);
        client.finalize_round(&round_id, &ids);
        assert_round_event(&env, symbol_short!("payout"));
        assert_round_event(&env, symbol_short!("finalized"));

        // p1: sqrt(3)=1, p2: sqrt(1)=1 → equal split of 1000 = 500 each
        // p1 payout = 300 contributions + 500 matching = 800
        // p2 payout = 100 contributions + 500 matching = 600
        let tok_client = TokenClient::new(&env, &token_addr);
        assert_eq!(tok_client.balance(&owner1), 800);
        assert_eq!(tok_client.balance(&owner2), 600);
        assert_eq!(client.get_round(&round_id).status, RoundStatus::Finalized);
    }

    #[test]
    fn contributor_count_unique_only() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        mint(&env, &token_addr, &admin, 1_000);
        let contributor = Address::generate(&env);
        mint(&env, &token_addr, &contributor, 500);

        let owner = Address::generate(&env);
        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        let proj_id = register_project(&env, &client, round_id, &owner);

        client.contribute(&contributor, &round_id, &proj_id, &100i128);
        client.contribute(&contributor, &round_id, &proj_id, &100i128);

        let p = client.get_project(&round_id, &proj_id);
        assert_eq!(p.contributor_count, 1); // still 1 unique contributor
        assert_eq!(p.total_contributions, 200);
    }

    #[test]
    fn finalize_zero_contributors_distributes_no_matching() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        mint(&env, &token_addr, &admin, 1_000);
        let owner = Address::generate(&env);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        let project_id = register_project(&env, &client, round_id, &owner);
        let ids = project_ids(&env, &[project_id]);

        client.finalize_round(&round_id, &ids);

        let project = client.get_project(&round_id, &project_id);
        assert_eq!(project.matching_amount, 0);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&owner), 0);
        assert_eq!(client.get_round(&round_id).status, RoundStatus::Finalized);
    }

    #[test]
    fn single_project_receives_full_matching_pool() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner = Address::generate(&env);
        let contributor = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_000);
        mint(&env, &token_addr, &contributor, 100);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        let project_id = register_project(&env, &client, round_id, &owner);
        client.contribute(&contributor, &round_id, &project_id, &100i128);

        let ids = project_ids(&env, &[project_id]);
        client.finalize_round(&round_id, &ids);

        let project = client.get_project(&round_id, &project_id);
        assert_eq!(project.matching_amount, 1_000);
        assert_eq!(TokenClient::new(&env, &token_addr).balance(&owner), 1_100);
    }

    #[test]
    #[should_panic(expected = "outside round window")]
    fn contribute_rejects_before_round_window() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner = Address::generate(&env);
        let contributor = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_000);
        mint(&env, &token_addr, &contributor, 100);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq + 10, seq + 20);
        let project_id = register_project(&env, &client, round_id, &owner);

        client.contribute(&contributor, &round_id, &project_id, &100i128);
    }

    #[test]
    #[should_panic(expected = "outside round window")]
    fn contribute_rejects_after_round_window() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner = Address::generate(&env);
        let contributor = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_000);
        mint(&env, &token_addr, &contributor, 100);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 5);
        let project_id = register_project(&env, &client, round_id, &owner);
        env.ledger().with_mut(|ledger| ledger.sequence_number = seq + 6);

        client.contribute(&contributor, &round_id, &project_id, &100i128);
    }

    #[test]
    #[should_panic(expected = "round not active")]
    fn register_project_rejects_finalized_round() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_000);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        let project_id = register_project(&env, &client, round_id, &owner);
        let ids = project_ids(&env, &[project_id]);
        client.finalize_round(&round_id, &ids);

        client.register_project(
            &round_id,
            &owner,
            &String::from_str(&env, "Late project"),
            &String::from_str(&env, "Desc"),
        );
    }

    #[test]
    #[should_panic(expected = "round not active")]
    fn contribute_rejects_finalized_round() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner = Address::generate(&env);
        let contributor = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_000);
        mint(&env, &token_addr, &contributor, 100);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        let project_id = register_project(&env, &client, round_id, &owner);
        let ids = project_ids(&env, &[project_id]);
        client.finalize_round(&round_id, &ids);

        client.contribute(&contributor, &round_id, &project_id, &100i128);
    }

    #[test]
    #[should_panic]
    fn finalize_round_requires_admin_auth() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner = Address::generate(&env);
        let caller = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_000);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_000, seq, seq + 100);
        let project_id = register_project(&env, &client, round_id, &owner);
        let ids = project_ids(&env, &[project_id]);

        client
            .mock_auths(&[MockAuth {
                address: &caller,
                invoke: &MockAuthInvoke {
                    contract: &client.address,
                    fn_name: "finalize_round",
                    args: (&round_id, &ids).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .finalize_round(&round_id, &ids);
    }

    #[test]
    fn matching_pool_remainder_is_left_as_dust() {
        let (env, client, admin) = setup();
        let token_addr = make_token(&env, &admin);
        let owner1 = Address::generate(&env);
        let owner2 = Address::generate(&env);
        let c1 = Address::generate(&env);
        let c2 = Address::generate(&env);
        mint(&env, &token_addr, &admin, 1_001);
        mint(&env, &token_addr, &c1, 100);
        mint(&env, &token_addr, &c2, 100);

        let seq = env.ledger().sequence();
        let round_id = create_round(&env, &client, &admin, &token_addr, 1_001, seq, seq + 100);
        let p1 = register_project(&env, &client, round_id, &owner1);
        let p2 = register_project(&env, &client, round_id, &owner2);
        client.contribute(&c1, &round_id, &p1, &100i128);
        client.contribute(&c2, &round_id, &p2, &100i128);

        let ids = project_ids(&env, &[p1, p2]);
        client.finalize_round(&round_id, &ids);

        assert_eq!(client.get_project(&round_id, &p1).matching_amount, 500);
        assert_eq!(client.get_project(&round_id, &p2).matching_amount, 500);
        let tok = TokenClient::new(&env, &token_addr);
        assert_eq!(tok.balance(&owner1), 600);
        assert_eq!(tok.balance(&owner2), 600);
    }
}
