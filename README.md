# soroban-crowdfunding

[![Stellar](https://img.shields.io/badge/Stellar-Network-blue?logo=stellar)](https://stellar.org)
[![Soroban](https://img.shields.io/badge/Soroban-v21-blueviolet)](https://soroban.stellar.org)
[![Wave Program](https://img.shields.io/badge/Drips-Stellar%20Wave-orange)](https://www.drips.network/wave/stellar)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![CI](https://github.com/YOUR_USERNAME/soroban-crowdfunding/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR_USERNAME/soroban-crowdfunding/actions/workflows/ci.yml)

A production-ready, open-source decentralized crowdfunding and public goods funding protocol built on [Stellar](https://stellar.org) using [Soroban](https://soroban.stellar.org) smart contracts (SDK v21).

## Contracts

| Contract | Path | Description | Status |
|---|---|---|---|
| **Campaign** | `contracts/campaign` | Donation campaigns with goal tracking, fee splits, and refunds | ✅ Production |
| **Grants** | `contracts/grants` | Milestone-based grants with evidence submission and fund locking | ✅ Production |
| **Quadratic** | `contracts/quadratic` | Quadratic funding rounds with matching pool distribution | ✅ Production |

## Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Stellar CLI
cargo install --locked stellar-cli --features opt
```

### Build

```bash
git clone https://github.com/YOUR_USERNAME/soroban-crowdfunding
cd soroban-crowdfunding

# Build native (for tests)
cargo build

# Build WASM (for deployment)
cargo build --target wasm32-unknown-unknown --release
```

### Test

```bash
cargo test
```

## Project Structure

```
soroban-crowdfunding/
├── Cargo.toml                        # Workspace root
├── contracts/
│   ├── campaign/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs               # Campaign contract
│   ├── grants/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs               # Grants contract
│   └── quadratic/
│       ├── Cargo.toml
│       └── src/lib.rs               # Quadratic funding contract
├── docs/
│   └── WAVE_ISSUES.md               # Open issues for contributors
├── .github/
│   └── workflows/
│       └── ci.yml                   # GitHub Actions CI
├── CONTRIBUTING.md
└── LICENSE
```

## Use Cases

### Campaigns
Open or curated donation campaigns with configurable goals, deadlines, and platform fees. Supports USDC, XLM, or any Stellar asset. Donors are protected by automatic refunds if a campaign fails to reach its goal.

### Grants
Milestone-based funding for builders and teams. Grantors lock the full amount upfront; funds are released incrementally as milestones are approved. Grantors can revoke and reclaim undisbursed funds at any time.

### Quadratic Funding
Democratic public goods funding where the matching pool is distributed proportionally to the square root of unique contributor counts — amplifying projects with broad community support over those with a few large donors.

### Supported Tokens
- **USDC** — Stellar USDC (Circle)
- **XLM** — Native Stellar Lumens (wrapped as Soroban token)
- Any SEP-41 compliant Stellar asset

## Contributing

Contributions are welcome! This project participates in the **[Stellar Wave Program](https://www.drips.network/wave/stellar)** on Drips Network. See [CONTRIBUTING.md](CONTRIBUTING.md) for bounty details and open issues in [docs/WAVE_ISSUES.md](docs/WAVE_ISSUES.md).

## License

[MIT](LICENSE) © 2026
