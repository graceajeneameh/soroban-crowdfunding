# Testnet deployment script

`deploy_testnet.sh` builds the contracts, deploys them to Stellar Testnet,
initializes them, adds example categories, and creates a small end-to-end data
set:

1. An open campaign using the `DeFi` category and a 1,000-unit goal.
2. A two-milestone grant totaling 500 units.
3. A quadratic funding round with a 2,000-unit matching pool.
4. Two registered quadratic projects.

## Prerequisites

- Rust with the Stellar-compatible WASM target:

  ```bash
  rustup target add wasm32v1-none
  ```

- Stellar CLI installed and available as `stellar`:

  ```bash
  cargo install --locked stellar-cli
  ```

## Usage

From the repository root:

```bash
chmod +x scripts/deploy_testnet.sh
./scripts/deploy_testnet.sh
```

By default, the script:

- uses the `testnet` network
- creates or reuses a funded identity named `crowdfunding-testnet-deployer`
- creates a testnet asset named `USDC` issued by the deployer account
- deploys fresh contract instances on each run using timestamped aliases
- prints contract IDs and transaction links at the end

The default deployer-issued `USDC` asset keeps the script self-contained:
because the deployer is the issuer, the grant and quadratic examples can lock
funds without requiring a manual faucet or trustline setup.

## Configuration

All configuration is provided through environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `NETWORK` | `testnet` | Stellar CLI network name |
| `STELLAR_IDENTITY` | `crowdfunding-testnet-deployer` | Local Stellar CLI identity |
| `TOKEN_ASSET` | `USDC:<deployer-address>` | Stellar asset wrapped as the example token |
| `TOKEN_CONTRACT_ID` | empty | Use an existing token contract instead of deploying an asset contract |
| `TOKEN_ALIAS` | `crowdfunding_usdc` | Alias for the token contract |
| `RUN_ID` | current UTC timestamp | Suffix used for the default fresh contract aliases |
| `CAMPAIGN_ALIAS` | `crowdfunding_campaign_<RUN_ID>` | Alias for the Campaign contract |
| `GRANTS_ALIAS` | `crowdfunding_grants_<RUN_ID>` | Alias for the Grants contract |
| `QUADRATIC_ALIAS` | `crowdfunding_quadratic_<RUN_ID>` | Alias for the Quadratic contract |
| `WASM_TARGET` | `wasm32v1-none` | Rust target used for Stellar-compatible WASM |
| `FEE_BPS` | `250` | Campaign platform fee in basis points |
| `CAMPAIGN_GOAL` | `1000` | Example campaign goal |
| `GRANT_TOTAL` | `500` | Example grant total; split across two milestones |
| `ROUND_POOL` | `2000` | Example quadratic matching pool |
| `ROUND_DURATION_LEDGERS` | `1000` | Example round duration from the configured start ledger |
| `DEADLINE_LEDGER` | `429496729` | Example campaign deadline ledger |
| `ROUND_START_LEDGER` | latest testnet ledger | Example quadratic round start ledger |

Example using a pre-existing token contract:

```bash
TOKEN_CONTRACT_ID=CDLZFC3SYJYDZT7K67VZ75HPJVIEUVZPHCE7AKQHOUWH2DJ4XKCRJ5QG \
  ./scripts/deploy_testnet.sh
```

## Output

At the end of a successful run the script prints:

- network
- identity
- deployer account
- token asset and contract ID
- Campaign, Grants, and Quadratic contract IDs
- example campaign, grant, round, and project IDs
- example categories
- transaction links emitted by Stellar CLI
