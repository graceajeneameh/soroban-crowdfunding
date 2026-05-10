# Wave Issues — Open Bounties

Copy-paste each issue below directly into GitHub Issues. All issues are eligible for the [Stellar Wave Program](https://www.drips.network/wave/stellar) on Drips Network.

---

## Issue 1 — Full test suite for Campaign contract

**Title:** `test: full test suite for Campaign contract`

**Labels:** `testing`, `medium`, `good first issue`

**Description:**
The Campaign contract has basic lifecycle tests but is missing coverage for edge cases, error paths, and event emissions. A comprehensive test suite is needed to ensure production readiness.

**Tasks:**
- [ ] Test `create_campaign` with invalid goal (0 or negative)
- [ ] Test `create_campaign` with deadline in the past
- [ ] Test `donate` to a Pending (unapproved Curated) campaign — must panic
- [ ] Test `donate` after deadline — must panic
- [ ] Test `withdraw` on a non-Successful campaign — must panic
- [ ] Test `mark_failed` before deadline — must panic
- [ ] Test `mark_failed` when goal was reached — must panic
- [ ] Test `refund` double-claim prevention
- [ ] Test `cancel_campaign` with existing donations — must panic
- [ ] Test platform fee calculation at 0 bps, 250 bps, and 1000 bps
- [ ] Verify all events are emitted with correct payloads

**Acceptance Criteria:**
- All new tests pass with `cargo test`
- No existing tests broken
- `cargo clippy` reports zero warnings on test code

**Complexity:** Medium — 150 pts

---

## Issue 2 — Full test suite for Grants contract

**Title:** `test: full test suite for Grants contract`

**Labels:** `testing`, `medium`

**Description:**
The Grants contract needs comprehensive tests covering all milestone state transitions, authorization checks, and edge cases.

**Tasks:**
- [ ] Test `create_grant` with empty milestones vec — must panic
- [ ] Test `create_grant` with zero total amount — must panic
- [ ] Test `submit_milestone` by non-grantee — must panic
- [ ] Test `approve_milestone` on a non-Submitted milestone — must panic
- [ ] Test `reject_milestone` clears evidence string
- [ ] Test `revoke_grant` on a Completed grant — must panic
- [ ] Test `revoke_grant` when all funds already disbursed (remaining = 0)
- [ ] Test auto-complete triggers when last milestone approved
- [ ] Test that grantor cannot call `submit_milestone`
- [ ] Verify all events emitted with correct payloads

**Acceptance Criteria:**
- All new tests pass with `cargo test`
- No existing tests broken
- `cargo clippy` reports zero warnings

**Complexity:** Medium — 150 pts

---

## Issue 3 — Full test suite for Quadratic contract

**Title:** `test: full test suite for Quadratic contract`

**Labels:** `testing`, `medium`

**Description:**
The Quadratic contract needs tests for edge cases in the matching algorithm, round lifecycle, and authorization.

**Tasks:**
- [ ] Test `finalize_round` with zero contributors across all projects (sqrt_sum = 0, no matching distributed)
- [ ] Test `finalize_round` with a single project receiving full matching pool
- [ ] Test `contribute` outside round window (before start_ledger and after end_ledger) — must panic
- [ ] Test `register_project` on a Finalized round — must panic
- [ ] Test `contribute` to a Finalized round — must panic
- [ ] Test `finalize_round` by non-admin — must panic
- [ ] Test `isqrt` with large values (e.g., 1_000_000)
- [ ] Test matching pool remainder handling (integer division dust)
- [ ] Verify all events emitted with correct payloads

**Acceptance Criteria:**
- All new tests pass with `cargo test`
- No existing tests broken
- `cargo clippy` reports zero warnings

**Complexity:** Medium — 150 pts

---

## Issue 4 — Campaign update and progress tracking

**Title:** `feat: campaign update and progress tracking`

**Labels:** `enhancement`, `high`

**Description:**
Campaign creators need the ability to post updates and milestones to keep donors informed. Donors should be able to query progress history on-chain.

**Tasks:**
- [ ] Add `CampaignUpdate` struct: `{ campaign_id, author, message, ledger }`
- [ ] Add `post_update(env, caller, campaign_id, message)` — callable by creator only
- [ ] Store updates in `env.storage().persistent()` keyed by `(campaign_id, update_index)`
- [ ] Add `update_count` field to `Campaign` struct
- [ ] Add `get_update(env, campaign_id, index) -> CampaignUpdate` view
- [ ] Emit `("campaign", "update")` event on each post
- [ ] Add tests: post update, get update, non-creator post panics

**Acceptance Criteria:**
- Creator can post updates on Active and Successful campaigns
- Non-creator callers are rejected with auth error
- Updates are retrievable by index
- All tests pass, clippy clean

**Complexity:** High — 200 pts

---

## Issue 5 — Grant application system

**Title:** `feat: grant application system`

**Labels:** `enhancement`, `high`

**Description:**
Currently grantors must know the grantee address upfront. A grant application system would allow grantees to apply for open grants, and grantors to select applicants.

**Tasks:**
- [ ] Add `GrantApplication` struct: `{ grant_id, applicant, proposal, status: Applied/Accepted/Rejected }`
- [ ] Add `open_grant(env, grantor, token, total_amount, title, description, milestones)` — creates grant without locking funds, status `Open`
- [ ] Add `apply_for_grant(env, applicant, grant_id, proposal)` — stores application
- [ ] Add `accept_application(env, grant_id, applicant)` — grantor selects applicant, locks funds, sets grantee, transitions to Active
- [ ] Add `get_application(env, grant_id, applicant) -> GrantApplication` view
- [ ] Emit events on apply and accept
- [ ] Add tests for full application lifecycle

**Acceptance Criteria:**
- Grantor can create an open grant without locking funds
- Multiple applicants can apply
- Accepting an application locks funds and activates the grant
- All tests pass, clippy clean

**Complexity:** High — 200 pts

---

## Issue 6 — Donor leaderboard and contribution history

**Title:** `feat: donor leaderboard and contribution history`

**Labels:** `enhancement`, `high`

**Description:**
Add on-chain tracking of total donations per address across all campaigns, enabling a leaderboard and per-donor history queries.

**Tasks:**
- [ ] Add `DonorStats` struct: `{ donor, total_donated, campaign_count }`
- [ ] Update `donate` to increment `DonorStats` in `env.storage().persistent()` keyed by donor address
- [ ] Add `get_donor_stats(env, donor) -> DonorStats` view
- [ ] Add `get_donor_campaigns(env, donor) -> Vec<u64>` — returns list of campaign IDs donated to
- [ ] Store campaign list per donor in persistent storage
- [ ] Emit `("campaign", "donor_stats")` event when stats updated
- [ ] Add tests for stats accumulation across multiple campaigns

**Acceptance Criteria:**
- Stats correctly accumulate across multiple donations and campaigns
- Campaign list is deduplicated (one entry per campaign)
- All tests pass, clippy clean

**Complexity:** High — 200 pts

---

## Issue 7 — Quadratic round contributor whitelist

**Title:** `feat: quadratic round contributor whitelist`

**Labels:** `enhancement`, `medium`

**Description:**
Round admins should be able to restrict contributions to a whitelist of approved addresses (e.g., verified community members).

**Tasks:**
- [ ] Add optional `whitelist_enabled: bool` field to `Round`
- [ ] Add `add_to_whitelist(env, round_id, address)` — admin only
- [ ] Add `remove_from_whitelist(env, round_id, address)` — admin only
- [ ] Store whitelist in `env.storage().persistent()` keyed by `(round_id, address)`
- [ ] In `contribute`, check whitelist if `whitelist_enabled` — panic if not whitelisted
- [ ] Add `is_whitelisted(env, round_id, address) -> bool` view
- [ ] Add tests: whitelisted contributor succeeds, non-whitelisted panics, whitelist disabled allows all

**Acceptance Criteria:**
- Whitelist is opt-in per round
- Non-whitelisted contributors are rejected when whitelist is enabled
- All tests pass, clippy clean

**Complexity:** Medium — 150 pts

---

## Issue 8 — Campaign category registry

**Title:** `feat: campaign category registry`

**Labels:** `enhancement`, `medium`

**Description:**
Currently categories are free-form strings. A category registry would enforce valid categories and enable filtering campaigns by category.

**Tasks:**
- [ ] Add `add_category(env, admin, name: String)` — admin only, stores in instance storage
- [ ] Add `remove_category(env, admin, name: String)` — admin only
- [ ] Add `get_categories(env) -> Vec<String>` view
- [ ] In `create_campaign`, validate that `category` exists in the registry — panic if not
- [ ] Emit `("campaign", "category_added")` and `("campaign", "category_removed")` events
- [ ] Add tests: add category, create campaign with valid/invalid category, remove category

**Acceptance Criteria:**
- Only admin can manage categories
- `create_campaign` rejects unknown categories
- All tests pass, clippy clean

**Complexity:** Medium — 150 pts

---

## Issue 9 — Doc comments for all contracts

**Title:** `docs: add doc comments to all public types and functions`

**Labels:** `documentation`, `trivial`, `good first issue`

**Description:**
All public structs, enums, and contract functions across the three contracts are missing Rust doc comments (`///`). Adding them improves developer experience and enables `cargo doc` output.

**Tasks:**
- [ ] Add `///` doc comments to all public structs and their fields in `campaign/src/lib.rs`
- [ ] Add `///` doc comments to all public functions in `campaign/src/lib.rs`
- [ ] Add `///` doc comments to all public structs and their fields in `grants/src/lib.rs`
- [ ] Add `///` doc comments to all public functions in `grants/src/lib.rs`
- [ ] Add `///` doc comments to all public structs and their fields in `quadratic/src/lib.rs`
- [ ] Add `///` doc comments to all public functions in `quadratic/src/lib.rs`
- [ ] Verify `cargo doc --no-deps` builds without warnings

**Acceptance Criteria:**
- Every public item has a doc comment
- `cargo doc --no-deps` produces zero warnings
- No logic changes — documentation only

**Complexity:** Trivial — 100 pts

---

## Issue 10 — Testnet deploy script with example round and categories

**Title:** `script: testnet deploy script with example round and categories`

**Labels:** `tooling`, `medium`

**Description:**
There is no automated way to deploy and initialize the contracts on Stellar Testnet. A shell script using the Stellar CLI would lower the barrier for contributors and evaluators.

**Tasks:**
- [ ] Create `scripts/deploy_testnet.sh`
- [ ] Script generates or loads a funded testnet keypair
- [ ] Deploys all three WASM contracts using `stellar contract deploy`
- [ ] Calls `initialize` on each contract with sensible defaults
- [ ] Creates one example Campaign (Open type, USDC, 1000 goal)
- [ ] Creates one example Grant (2 milestones, 500 total)
- [ ] Creates one example Quadratic round (matching pool 2000, 2 projects registered)
- [ ] Adds example categories: `["DeFi", "Public Goods", "Tooling", "Education", "Art"]`
- [ ] Prints all deployed contract IDs and transaction hashes
- [ ] Add `scripts/README.md` documenting how to run the script

**Acceptance Criteria:**
- Script runs end-to-end on Stellar Testnet without manual intervention
- All contract IDs printed to stdout
- `scripts/README.md` documents prerequisites and usage
- Script is idempotent (re-running does not break existing state)

**Complexity:** Medium — 150 pts
