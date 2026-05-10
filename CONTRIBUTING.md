# Contributing to soroban-crowdfunding

Thank you for your interest in contributing! This project participates in the **[Stellar Wave Program](https://www.drips.network/wave/stellar)** on Drips Network, which rewards open-source contributors with on-chain streaming payments.

## Dev Setup

```bash
# 1. Install Rust stable
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# 2. Add WASM target
rustup target add wasm32-unknown-unknown

# 3. Install Stellar CLI
cargo install --locked stellar-cli --features opt

# 4. Clone and build
git clone https://github.com/YOUR_USERNAME/soroban-crowdfunding
cd soroban-crowdfunding
cargo build
```

## Wave Bounty Table

Issues are tagged with complexity labels. Merged PRs earn points redeemable through the Drips Stellar Wave Program.

| Complexity | Points | Examples |
|---|---|---|
| **Trivial** | 100 pts | Doc comments, typo fixes, minor refactors |
| **Medium** | 150 pts | New features, test suites, scripts |
| **High** | 200 pts | New subsystems, security improvements, integrations |

See [docs/WAVE_ISSUES.md](docs/WAVE_ISSUES.md) for the full list of open issues ready to claim.

## PR Guidelines

1. Fork the repo and create a branch: `git checkout -b feat/your-feature`
2. Write tests for any new logic — all PRs must maintain or improve test coverage
3. Run lint and tests locally before pushing (see commands below)
4. Open a PR against `main` with a clear description of what changed and why
5. Reference the issue number in your PR description: `Closes #N`
6. PRs require at least one approving review before merge

## Commands

```bash
# Run all tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy (zero warnings policy)
cargo clippy --all-targets --all-features -- -D warnings

# Build WASM artifacts
cargo build --target wasm32-unknown-unknown --release
```

## Code Style

- All contracts must use `#![no_std]`
- Use `env.storage().persistent()` for per-entity state
- Use `env.storage().instance()` for global config and counters
- Emit events via `env.events().publish()` on every state change
- Panic with descriptive messages on invalid state transitions
- Zero out balances before token transfers to prevent double-spend

## Questions

Open a GitHub Discussion or reach out in the [Stellar Developer Discord](https://discord.gg/stellardev).
