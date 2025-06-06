URL="http://localhost:3000"
NETWORK="preview"
INTERVAL=60
OFFER_TOKEN="."
OFFER_AMOUNT=1000000
RECEIVE_TOKEN="99b071ce8580d6a3a11b4902145adb8bfd0d2a03935af8cf66403e15.534245525259"
RECEIVE_AMOUNT_MIN=0
VALID_FOR_SECS=300
PROJECT_ID="1"
DISPLAY_NAME="My worker"

config=$(jq -n \
    --arg network "$NETWORK" \
    --argjson interval "$INTERVAL" \
    --arg offer_token "$OFFER_TOKEN" \
    --argjson offer_amount "$OFFER_AMOUNT" \
    --arg receive_token "$RECEIVE_TOKEN" \
    --argjson receive_amount_min "$RECEIVE_AMOUNT_MIN" \
    --argjson valid_for_secs "$VALID_FOR_SECS" \
    '$ARGS.named'
)
echo "$config"

spec=$(jq -n \
    --arg network "$NETWORK" \
    --arg operatorVersion "1" \
    --arg throughputTier "0" \
    --arg displayName "$DISPLAY_NAME" \
    --arg url "file:///workers/dollar-cost-average.wasm" \
    --argjson config "$config" \
    --arg version "1" \
    '$ARGS.named'
)

payload=$(jq -n \
    --arg projectId "$PROJECT_ID" \
    --arg kind "BaliusWorker" \
    --arg spec "$spec" \
    '$ARGS.named'
)

response=$(curl -s "$URL/resources" -H 'Content-Type: application/json' -d "$payload")
echo $response | jq

payload=$(jq -n \
    --arg method "get-signer-key" \
    --argjson params "{}" \
    '$ARGS.named'
)
id=$(jq -r ".id" <(echo "$response"))
response=$(curl -s "$URL/worker/$id" -H 'Content-Type: application/json' -d "$payload")
echo $response | jq