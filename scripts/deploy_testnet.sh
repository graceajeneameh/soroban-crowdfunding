#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NETWORK="${NETWORK:-testnet}"
IDENTITY="${STELLAR_IDENTITY:-crowdfunding-testnet-deployer}"
TOKEN_ASSET="${TOKEN_ASSET:-}"
TOKEN_CONTRACT_ID="${TOKEN_CONTRACT_ID:-}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%d%H%M%S)}"
TOKEN_ALIAS="${TOKEN_ALIAS:-crowdfunding_usdc}"
CAMPAIGN_ALIAS="${CAMPAIGN_ALIAS:-crowdfunding_campaign_$RUN_ID}"
GRANTS_ALIAS="${GRANTS_ALIAS:-crowdfunding_grants_$RUN_ID}"
QUADRATIC_ALIAS="${QUADRATIC_ALIAS:-crowdfunding_quadratic_$RUN_ID}"
WASM_TARGET="${WASM_TARGET:-wasm32v1-none}"
FEE_BPS="${FEE_BPS:-250}"
CAMPAIGN_GOAL="${CAMPAIGN_GOAL:-1000}"
GRANT_TOTAL="${GRANT_TOTAL:-500}"
ROUND_POOL="${ROUND_POOL:-2000}"
ROUND_DURATION_LEDGERS="${ROUND_DURATION_LEDGERS:-1000}"
EXAMPLE_CATEGORIES=("DeFi" "Public Goods" "Tooling" "Education" "Art")

CAMPAIGN_WASM="$ROOT_DIR/target/$WASM_TARGET/release/campaign.wasm"
GRANTS_WASM="$ROOT_DIR/target/$WASM_TARGET/release/grants.wasm"
QUADRATIC_WASM="$ROOT_DIR/target/$WASM_TARGET/release/quadratic.wasm"

TX_LINKS=()
DEPLOYED_CONTRACT_ID=""

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

last_non_empty_line() {
  /usr/bin/awk 'NF { line = $0 } END { print line }'
}

record_tx_links() {
  while IFS= read -r line; do
    if [[ "$line" == *"stellar.expert/explorer/"*"/tx/"* ]]; then
      TX_LINKS+=("$line")
    fi
  done
}

run() {
  echo "+ $*" >&2
  "$@"
}

deploy_wasm() {
  local wasm_path="$1"
  local alias="$2"
  local output
  local contract_id

  if [[ ! -f "$wasm_path" ]]; then
    echo "Expected WASM artifact not found: $wasm_path" >&2
    exit 1
  fi

  if ! output="$(
    stellar contract deploy \
      --wasm "$wasm_path" \
      --source-account "$IDENTITY" \
      --network "$NETWORK" \
      --alias "$alias" 2>&1
  )"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi
  printf '%s\n' "$output"
  record_tx_links <<<"$output"
  contract_id="$(last_non_empty_line <<<"$output")"

  if [[ -z "$contract_id" || "$contract_id" != C* ]]; then
    echo "Deploy for $alias did not return a contract id" >&2
    exit 1
  fi

  DEPLOYED_CONTRACT_ID="$contract_id"
}

invoke() {
  local contract_id="$1"
  local output
  shift

  echo "+ stellar contract invoke --id $contract_id --source-account $IDENTITY --network $NETWORK --send=yes -- $*" >&2
  if ! output="$(
    stellar contract invoke \
      --id "$contract_id" \
      --source-account "$IDENTITY" \
      --network "$NETWORK" \
      --send=yes \
      -- "$@" 2>&1
  )"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi
  printf '%s\n' "$output"
  record_tx_links <<<"$output"
}

need_cmd cargo
need_cmd stellar

cd "$ROOT_DIR"

echo "Building release WASM artifacts..."
run cargo build --target "$WASM_TARGET" --release --workspace

if stellar keys address "$IDENTITY" >/dev/null 2>&1; then
  echo "Using existing Stellar identity: $IDENTITY"
else
  echo "Generating and funding Stellar testnet identity: $IDENTITY"
  run stellar keys generate "$IDENTITY" --network "$NETWORK" --fund
fi

DEPLOYER_ADDRESS="$(stellar keys address "$IDENTITY")"
echo "Deployer address: $DEPLOYER_ADDRESS"

if [[ -z "$TOKEN_ASSET" ]]; then
  TOKEN_ASSET="USDC:$DEPLOYER_ADDRESS"
fi

if [[ -z "$TOKEN_CONTRACT_ID" ]]; then
  echo "Resolving Stellar Asset Contract for $TOKEN_ASSET..."
  ASSET_DEPLOY_OUTPUT="$(mktemp)"
  if stellar contract asset deploy \
    --asset "$TOKEN_ASSET" \
    --source-account "$IDENTITY" \
    --network "$NETWORK" \
    --alias "$TOKEN_ALIAS" >"$ASSET_DEPLOY_OUTPUT" 2>&1; then
    /bin/cat "$ASSET_DEPLOY_OUTPUT"
    record_tx_links <"$ASSET_DEPLOY_OUTPUT"
    TOKEN_CONTRACT_ID="$(last_non_empty_line <"$ASSET_DEPLOY_OUTPUT")"
  else
    /bin/cat "$ASSET_DEPLOY_OUTPUT" >&2
    if /usr/bin/grep -q "ExistingValue\\|contract already exists" "$ASSET_DEPLOY_OUTPUT"; then
      echo "Asset contract already exists; deriving deterministic contract ID."
      TOKEN_CONTRACT_ID="$(
        stellar contract id asset \
          --asset "$TOKEN_ASSET" \
          --network "$NETWORK" \
          | last_non_empty_line
      )"
    else
      /bin/rm -f "$ASSET_DEPLOY_OUTPUT"
      exit 1
    fi
  fi
  /bin/rm -f "$ASSET_DEPLOY_OUTPUT"
fi
echo "Token contract ID: $TOKEN_CONTRACT_ID"

echo "Deploying crowdfunding contracts..."
deploy_wasm "$CAMPAIGN_WASM" "$CAMPAIGN_ALIAS"
CAMPAIGN_ID="$DEPLOYED_CONTRACT_ID"
deploy_wasm "$GRANTS_WASM" "$GRANTS_ALIAS"
GRANTS_ID="$DEPLOYED_CONTRACT_ID"
deploy_wasm "$QUADRATIC_WASM" "$QUADRATIC_ALIAS"
QUADRATIC_ID="$DEPLOYED_CONTRACT_ID"

echo "Initializing contracts..."
invoke "$CAMPAIGN_ID" initialize --admin "$DEPLOYER_ADDRESS" --fee_bps "$FEE_BPS"
invoke "$GRANTS_ID" initialize --admin "$DEPLOYER_ADDRESS"
invoke "$QUADRATIC_ID" initialize --admin "$DEPLOYER_ADDRESS"

echo "Adding example categories..."
for category in "${EXAMPLE_CATEGORIES[@]}"; do
  invoke "$CAMPAIGN_ID" add_category --admin "$DEPLOYER_ADDRESS" --name "$category"
done

LATEST_LEDGER="$(
  stellar ledger latest --network "$NETWORK" --output json 2>/dev/null \
    | /usr/bin/awk -F'[:,]' '/"sequence"/ { gsub(/[^0-9]/, "", $6); print $6; exit }'
)"
if [[ -z "$LATEST_LEDGER" ]]; then
  LATEST_LEDGER=0
fi
DEADLINE_LEDGER="${DEADLINE_LEDGER:-429496729}"
ROUND_START_LEDGER="${ROUND_START_LEDGER:-$LATEST_LEDGER}"
ROUND_END_LEDGER="${ROUND_END_LEDGER:-$((ROUND_START_LEDGER + ROUND_DURATION_LEDGERS))}"

echo "Creating example open campaign..."
invoke "$CAMPAIGN_ID" create_campaign \
  --creator "$DEPLOYER_ADDRESS" \
  --token "$TOKEN_CONTRACT_ID" \
  --goal "$CAMPAIGN_GOAL" \
  --title "Example Open Campaign" \
  --description "Automated testnet campaign created by scripts/deploy_testnet.sh" \
  --category "DeFi" \
  --campaign_type Open \
  --deadline_ledger "$DEADLINE_LEDGER"

echo "Creating example grant with two milestones..."
FIRST_MILESTONE_AMOUNT=$((GRANT_TOTAL / 2))
SECOND_MILESTONE_AMOUNT=$((GRANT_TOTAL - FIRST_MILESTONE_AMOUNT))
MILESTONES="[{\"index\":0,\"description\":\"Design and setup\",\"amount\":\"$FIRST_MILESTONE_AMOUNT\",\"evidence\":\"\",\"status\":\"Pending\"},{\"index\":1,\"description\":\"Delivery and demo\",\"amount\":\"$SECOND_MILESTONE_AMOUNT\",\"evidence\":\"\",\"status\":\"Pending\"}]"
invoke "$GRANTS_ID" create_grant \
  --grantor "$DEPLOYER_ADDRESS" \
  --grantee "$DEPLOYER_ADDRESS" \
  --token "$TOKEN_CONTRACT_ID" \
  --title "Example Grant" \
  --description "Automated two-milestone grant created by scripts/deploy_testnet.sh" \
  --milestones "$MILESTONES"

echo "Creating example quadratic funding round..."
invoke "$QUADRATIC_ID" create_round \
  --admin "$DEPLOYER_ADDRESS" \
  --token "$TOKEN_CONTRACT_ID" \
  --matching_pool "$ROUND_POOL" \
  --title "Example Quadratic Round" \
  --description "Automated round created by scripts/deploy_testnet.sh" \
  --start_ledger "$ROUND_START_LEDGER" \
  --end_ledger "$ROUND_END_LEDGER"

echo "Registering two example projects..."
invoke "$QUADRATIC_ID" register_project \
  --round_id 0 \
  --owner "$DEPLOYER_ADDRESS" \
  --title "Public Goods Toolkit" \
  --description "Example Tooling project"
invoke "$QUADRATIC_ID" register_project \
  --round_id 0 \
  --owner "$DEPLOYER_ADDRESS" \
  --title "Education Grants" \
  --description "Example Education project"

cat <<SUMMARY

Deployment complete.

Network:             $NETWORK
Identity:            $IDENTITY
Deployer:            $DEPLOYER_ADDRESS
Token asset:         $TOKEN_ASSET
Token contract:      $TOKEN_CONTRACT_ID
Campaign contract:   $CAMPAIGN_ID
Grants contract:     $GRANTS_ID
Quadratic contract:  $QUADRATIC_ID
Example campaign id: 0
Example grant id:    0
Example round id:    0
Example projects:    0, 1
Categories:          ${EXAMPLE_CATEGORIES[*]}

Transaction links:
SUMMARY

for link in "${TX_LINKS[@]}"; do
  printf -- '- %s\n' "$link"
done
