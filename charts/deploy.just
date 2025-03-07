mod ibc-test

##############################################
## Deploying and Running using Helm and K8s ##
##############################################
defaultNamespace := "astria-dev-cluster"
deploy tool *ARGS:
  @just deploy-{{tool}} {{ARGS}}

delete tool *ARGS:
  @just delete-{{tool}} {{ARGS}}

init tool *ARGS:
  @just init-{{tool}} {{ARGS}}

run-smoke type *ARGS:
  @just run-smoke-{{type}} {{ARGS}}

load-image image_name tag=default_docker_tag repo=default_repo_name:
  kind load docker-image {{repo}}/{{image_name}}:{{tag}} --name astria-dev-cluster

deploy-all: deploy-cluster deploy-ingress-controller wait-for-ingress-controller deploy-astria-local wait-for-sequencer deploy-astrotrek (deploy-chart "sequencer-faucet") deploy-rollup
delete-all: clean clean-persisted-data

deploy-astria-local namespace=defaultNamespace: (deploy-chart "celestia-local" namespace) (deploy-sequencer)
delete-astria-local namespace=defaultNamespace: (delete-chart "celestia-local" namespace) (delete-sequencer)

[private]
deploy-chart chart namespace=defaultNamespace:
  helm install {{chart}}-chart ./charts/{{chart}} --namespace {{namespace}} --create-namespace

[private]
delete-chart chart namespace=defaultNamespace:
  helm uninstall {{chart}}-chart --namespace {{namespace}}

[private]
helm-add-if-not-exist repo url:
  helm repo list | grep -q {{repo}} || helm repo add {{repo}} {{url}}

hubbleEnabled := "false"
deploy-cluster namespace=defaultNamespace enableHubble=hubbleEnabled:
  kind create cluster --image kindest/node:v1.28.9 --config ./dev/kubernetes/kind-cluster-config.yml
  @just helm-add-if-not-exist cilium https://helm.cilium.io/ > /dev/null
  helm install cilium cilium/cilium --version 1.15.5 \
      -f ./dev/values/cilium.yml \
      {{ if enableHubble == "true" { '--set hubble.enabled=true --set hubble.relay.enabled=true --set hubble.ui.enabled=true --set hubble.ui.ingress.enabled=true' } else { '' } }} \
      --namespace kube-system
  kubectl create namespace {{namespace}}

deploy-metrics-server:
  kubectl apply -f ./dev/kubernetes/metrics-server-local.yml

deploy-ingress-controller:
  kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml

deploy-graph-node namespace=defaultNamespace: (deploy-chart "graph-node" namespace)
delete-graph-node namespace=defaultNamespace: (delete-chart "graph-node" namespace)
delete-graph-node-pvc namespace=defaultNamespace:
  kubectl delete pvc -n namespace \
    -l 'app.kubernetes.io/instance=graph-node-chart' \
    -o name | xargs -r kubectl delete -n ${namespace} && \
  kubectl delete pvc -n namespace \
    -l 'app.kubernetes.io/managed-by=Helm' \
    --field-selector 'metadata.name in (ipfs-pvc,postgres-pvc)'

[private]
deploy-celestia-local namespace=defaultNamespace: (deploy-chart "celestia-local" namespace)

[private]
delete-celestia-local namespace=defaultNamespace: (delete-chart "celestia-local" namespace)

deploy-secrets-store:
  @just helm-add-if-not-exist secrets-store-csi-driver https://kubernetes-sigs.github.io/secrets-store-csi-driver/charts
  helm install csi-secrets-store secrets-store-csi-driver/secrets-store-csi-driver --namespace kube-system

delete-secrets-store:
  @just delete chart csi-secrets-store kube-system

wait-for-ingress-controller:
  while ! kubectl wait --namespace ingress-nginx --for=condition=ready pod --selector=app.kubernetes.io/component=controller --timeout=600s; do \
    sleep 1; \
  done

validatorName := "single"
deploy-sequencer name=validatorName:
  helm dependency update charts/sequencer > /dev/null
  helm install \
    -f dev/values/validators/all.yml \
    -f dev/values/validators/{{name}}.yml \
    -n astria-validator-{{name}} --create-namespace \
    {{name}}-sequencer-chart ./charts/sequencer
deploy-sequencers: (deploy-sequencer "node0") (deploy-sequencer "node1") (deploy-sequencer "node2")

deploy-astrotrek:
  helm dependency update charts/astrotrek > /dev/null
  helm install astrotrek-chart ./charts/astrotrek \
    -n astria-dev-cluster \

delete-astrotrek:
  @just delete chart astrotrek 

deploy-hermes-local:
  helm install hermes-local-chart ./charts/hermes \
    -n astria-dev-cluster \
    -f dev/values/hermes/local.yaml
delete-hermes-local:
  @just delete chart hermes-local

delete-sequencer name=validatorName:
  @just delete chart {{name}}-sequencer astria-validator-{{name}}
delete-sequencers: (delete-sequencer "node0") (delete-sequencer "node1") (delete-sequencer "node2")

wait-for-sequencer:
  kubectl rollout status --watch statefulset/celestia-local -n astria-dev-cluster --timeout=600s
  kubectl rollout status --watch statefulset/sequencer -n astria-dev-cluster --timeout=600s

deploy-rollup: deploy-dev-rollup wait-for-rollup
wait-for-rollup: wait-for-dev-rollup wait-for-bridge-withdrawer
delete-rollup: delete-dev-rollup

defaultRollupName          := "astria"
defaultNetworkId           := ""
defaultPrivateKey          := ""
defaultSequencerStartBlock := ""
deploy-dev-rollup rollupName=defaultRollupName networkId=defaultNetworkId:
  helm dependency update charts/evm-stack > /dev/null
  helm install \
    {{ if rollupName          != '' { replace('--set config.rollup.name=# --set celestia-node.config.labelPrefix=#', '#', rollupName) } else { '' } }} \
    {{ if networkId           != '' { replace('--set config.rollup.networkId=#', '#', networkId) } else { '' } }} \
    -f dev/values/rollup/dev.yaml \
    {{rollupName}}-chain-chart ./charts/evm-stack --namespace astria-dev-cluster

deploy-flame-dev-rollup rollupName=defaultRollupName networkId=defaultNetworkId:
  helm dependency update charts/evm-stack > /dev/null
  helm install \
    {{ if rollupName          != '' { replace('--set config.rollup.name=# --set celestia-node.config.labelPrefix=#', '#', rollupName) } else { '' } }} \
    {{ if networkId           != '' { replace('--set config.rollup.networkId=#', '#', networkId) } else { '' } }} \
    -f dev/values/rollup/flame-dev.yaml \
    {{rollupName}}-chain-chart ./charts/evm-stack --namespace astria-dev-cluster

delete-dev-rollup rollupName=defaultRollupName:
  @just delete chart {{rollupName}}-chain

delete-flame-dev-rollup rollupName=defaultRollupName:
  @just delete-dev-rollup

wait-for-dev-rollup rollupName=defaultRollupName:
  kubectl rollout status --watch statefulset/{{rollupName}}-geth -n astria-dev-cluster --timeout=600s

deploy-auctioneer:
  helm dependency update charts/auctioneer > /dev/null
  helm install auctioneer-chart ./charts/auctioneer \
      --namespace astria-dev-cluster \
      -f dev/values/auctioneer/values.yaml

delete-auctioneer:
  @just delete chart auctioneer astria-dev-cluster

deploy-bridge-withdrawer:
  helm install evm-bridge-withdrawer-chart ./charts/evm-bridge-withdrawer \
    --namespace astria-dev-cluster \
    -f dev/values/withdrawer/values.yaml

delete-bridge-withdrawer:
  @just delete chart evm-bridge-withdrawer astria-dev-cluster

wait-for-bridge-withdrawer:
  kubectl wait -n astria-dev-cluster deployment evm-bridge-withdrawer-local --for=condition=Available=True --timeout=600s

defaultHypAgentConfig         := ""
defaultHypRelayerPrivateKey   := ""
defaultHypValidatorPrivateKey := ""
deploy-hyperlane-agents rollupName=defaultRollupName agentConfig=defaultHypAgentConfig relayerPrivateKey=defaultHypRelayerPrivateKey validatorPrivateKey=defaultHypValidatorPrivateKey:
  helm install --debug \
    {{ if rollupName          != '' { replace('--set config.name=# --set global.namespace=#-dev-cluster', '#', rollupName) } else { '' } }} \
    {{ if agentConfig         != '' { replace('--set config.agentConfig=#', '#', agentConfig) } else { '' } }} \
    {{ if relayerPrivateKey   != '' { replace('--set config.relayer.privateKey=#', '#', relayerPrivateKey) } else { '' } }} \
    {{ if validatorPrivateKey != '' { replace('--set config.validator.privateKey=#', '#', validatorPrivateKey) } else { '' } }} \
    {{rollupName}}-hyperlane-agents-chart ./charts/hyperlane-agents --namespace astria-dev-cluster

delete-hyperlane-agents rollupName=defaultRollupName:
  @just delete {{rollupName}}-hyperlane-agents

clean:
  kind delete cluster --name astria-dev-cluster

clean-persisted-data:
  rm -r /tmp/astria

deploy-local-metrics:
  kubectl apply -f kubernetes/metrics-server-local.yml

defaultTag := ""
deploy-smoke-test tag=defaultTag:
  @echo "Deploying ingress controller..." && just deploy ingress-controller > /dev/null
  @just wait-for-ingress-controller > /dev/null
  @echo "Deploying local celestia instance..." && just deploy celestia-local > /dev/null
  @helm dependency update charts/sequencer > /dev/null
  @helm dependency update charts/evm-stack > /dev/null
  @echo "Setting up single astria sequencer..." && helm install \
    -n astria-validator-single single-sequencer-chart ./charts/sequencer \
    -f dev/values/validators/all.yml \
    -f dev/values/validators/single.yml \
    {{ if tag != '' { replace('--set images.sequencer.devTag=# --set sequencer-relayer.images.sequencerRelayer.devTag=#', '#', tag) } else { '' } }} \
    --create-namespace > /dev/null
  @just wait-for-sequencer > /dev/null
  @echo "Starting EVM rollup..." && helm install -n astria-dev-cluster astria-chain-chart ./charts/evm-stack -f dev/values/rollup/dev.yaml \
    {{ if tag != '' { replace('--set evm-rollup.images.conductor.devTag=# --set composer.images.composer.devTag=# --set evm-bridge-withdrawer.images.evmBridgeWithdrawer.devTag=#', '#', tag) } else { '' } }} \
    --set blockscout-stack.enabled=false \
    --set postgresql.enabled=false \
    --set evm-faucet.enabled=false > /dev/null
  @just wait-for-rollup > /dev/null

deploy-smoke-cli tag=defaultTag:
  @echo "Deploying ingress controller..." && just deploy ingress-controller > /dev/null
  @just wait-for-ingress-controller > /dev/null
  @echo "Deploying local celestia instance..." && just deploy celestia-local > /dev/null
  @helm dependency update charts/sequencer > /dev/null
  @helm dependency update charts/evm-stack > /dev/null
  @echo "Setting up single astria sequencer..." && helm install \
    -n astria-validator-single single-sequencer-chart ./charts/sequencer \
    -f dev/values/validators/all.yml \
    -f dev/values/validators/single.yml \
    {{ if tag != '' { replace('--set images.sequencer.devTag=# --set sequencer-relayer.images.sequencerRelayer.devTag=#', '#', tag) } else { '' } }} \
    --create-namespace > /dev/null
  @just wait-for-sequencer > /dev/null
  @echo "Starting EVM rollup..." && helm install -n astria-dev-cluster astria-chain-chart ./charts/evm-stack -f dev/values/rollup/dev.yaml \
      {{ if tag != '' { replace('--set evm-rollup.images.conductor.devTag=# --set composer.images.composer.devTag=#', '#', tag) } else { '' } }} \
      --set blockscout-stack.enabled=false \
      --set postgresql.enabled=false \
      --set evm-bridge-withdrawer.enabled=false \
      --set evm-faucet.enabled=false > /dev/null
  @just wait-for-dev-rollup > /dev/null
  @sleep 10


evm_destination_address := "0xaC21B97d35Bf75A7dAb16f35b111a50e78A72F30"
# 1 RIA is 10^9 nRIA
sequencer_base_amount := "1000000000"
# RIA is 10^9, WEI is 10^18, 10^9 * 10^9 = 10^18
rollup_multiplier := "1000000000"
# 10 RIA
sequencer_transfer_amount := "10"
sequencer_rpc_url := "http://rpc.sequencer.localdev.me"
sequencer_bridge_address := "astria13ahqz4pjqfmynk9ylrqv4fwe4957x2p0h5782u"
sequencer_bridge_pkey := "dfa7108e38ab71f89f356c72afc38600d5758f11a8c337164713e4471411d2e0"
sequencer_chain_id := "sequencer-test-chain-0"
cli_image := "ghcr.io/astriaorg/astria-cli"
init-rollup-bridge tag=defaultTag rollupName=defaultRollupName evmDestinationAddress=evm_destination_address transferAmount=sequencer_transfer_amount:
  #!/usr/bin/env bash
  set -e

  SEQUENCER_FUNDS_PKEY="934ab488f9e1900f6a08f50605ce1409ca9d95ebdc400dafc2e8a4306419fd52"
  ASSET="nria"
  FEE_ASSET="nria"
  TRANSFER_AMOUNT=$(echo "{{transferAmount}} * {{sequencer_base_amount}}" | bc)

  IMAGE="{{cli_image}}{{ if tag != '' { replace(':#', '#', tag) } else { '' } }}"

  docker run --rm --network host $IMAGE sequencer init-bridge-account \
    --rollup-name {{rollupName}} \
    --private-key {{sequencer_bridge_pkey}} \
    --sequencer.chain-id {{sequencer_chain_id}} \
    --sequencer-url {{sequencer_rpc_url}} \
    --fee-asset=$FEE_ASSET --asset=$ASSET || exit 1
  docker run --rm --network host $IMAGE sequencer bridge-lock {{sequencer_bridge_address}} \
    --amount $TRANSFER_AMOUNT \
    --destination-chain-address {{evmDestinationAddress}} \
    --private-key $SEQUENCER_FUNDS_PKEY \
    --sequencer.chain-id {{sequencer_chain_id}} \
    --sequencer-url {{sequencer_rpc_url}} \
    --fee-asset=$FEE_ASSET --asset=$ASSET


init-ibc-bridge privateKey asset feeAsset tag=defaultTag rollupName=defaultRollupName:
  docker run --rm --network host {{cli_image}}{{ if tag != '' { replace(':#', '#', tag) } else { '' } }} sequencer init-bridge-account \
    --rollup-name {{ rollupName }} \
    --private-key {{ privateKey }} \
    --sequencer.chain-id {{ sequencer_chain_id }} \
    --sequencer-url {{ sequencer_rpc_url }} \
    --fee-asset {{ feeAsset }} \
    --asset {{ asset }}

eth_rpc_url := "http://executor.astria.localdev.me/"
eth_ws_url := "ws://ws-executor.astria.localdev.me/"
# bridge_tx_bytes is the tx to the withdraw smart contract on the evm.
# Uses private key for 0xaC21B97d35Bf75A7dAb16f35b111a50e78A72F30 to sign tx.
# was created via:
#  `forge script script/AstriaWithdrawer.s.sol:AstriaWithdrawerScript \
#        --rpc-url "http://executor.astria.localdev.me/" \
#        --legacy \
#        --broadcast \
#        --sig "withdrawToSequencer()" -vvvv`
# w/ values:
#  PRIVATE_KEY=0x8b3a7999072c9c9314c084044fe705db11714c6c4ed7cddb64da18ea270dd203
#  ASTRIA_WITHDRAWER=0xA58639fB5458e65E4fA917FF951C390292C24A15
#  SEQUENCER_DESTINATION_CHAIN_ADDRESS="astria17w0adeg64ky0daxwd2ugyuneellmjgnxl39504"
#  AMOUNT=1000000000000000000
bridge_tx_bytes := "0xf8f280843ba60f5782a35194a58639fb5458e65e4fa917ff951c390292c24a15880de0b6b3a7640000b884bab916d00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002d617374726961313777306164656736346b7930646178776432756779756e65656c6c6d6a676e786c333935303400000000000000000000000000000000000000820a95a034652da1bbcad94f6af3db785127dae70f9b4e7d4da3c3f4b36eafe7fce9bf58a0169ed71974bcd74f0cea148148b5f3f8da50cdd055057dd18a599a2a3e14679f"
bridge_tx_hash := "0x326c3910da4c96c5a40ba1505fc338164b659729f2f975ccb07e8794c96b66f6"
run-smoke-test tag=defaultTag:
  #!/usr/bin/env bash
  set -e
  ASTRIA_CLI_IMAGE="{{cli_image}}{{ if tag != '' { replace(':#', '#', tag) } else { '' } }}"
  MAX_CHECKS=30

  # Checking starting balance
  BALANCE=$(just evm-get-balance {{evm_destination_address}})
  if [ $BALANCE -ne 0 ]; then
    echo "Starting balance is not correct"
    exit 1
  fi

  echo "Testing Bridge In..."
  just init rollup-bridge {{tag}}
  CHECKS=0
  EXPECTED_BALANCE=$(echo "{{sequencer_transfer_amount}} * {{sequencer_base_amount}} * {{rollup_multiplier}}" | bc)
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    BALANCE=$(just evm-get-balance {{evm_destination_address}})
    echo "Check $CHECKS, Balance: $BALANCE, Expected: $EXPECTED_BALANCE"
    if [ "$BALANCE" == "$EXPECTED_BALANCE" ]; then
      echo "Bridge In success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Bridge In failure"
    exit 1
  fi

  echo "Testing Bridge Out..."
  just evm-send-raw-transaction {{bridge_tx_bytes}}
  TRANSFERED_BALANCE=$(echo "1 * {{sequencer_base_amount}} * {{rollup_multiplier}}" | bc)
  EXPECTED_BALANCE=$(echo "$EXPECTED_BALANCE - $TRANSFERED_BALANCE" | bc)
  CHECKS=0
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    BALANCE=$(just evm-get-balance {{evm_destination_address}})
    echo "Check $CHECKS, Balance: $BALANCE, Expected: $EXPECTED_BALANCE"
    if [ "$BALANCE" == "$EXPECTED_BALANCE" ]; then
      echo "Bridge Out EVM success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Bridge Out EVM failure"
    exit 1
  fi
  CHECKS=0
  EXPECTED_BALANCE=$(echo "1 * {{sequencer_base_amount}}" | bc)
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    BALANCE=$(docker run --rm --network host $ASTRIA_CLI_IMAGE sequencer account balance astria17w0adeg64ky0daxwd2ugyuneellmjgnxl39504 --sequencer-url {{sequencer_rpc_url}}  | awk '/nria/{print $(NF-1)}')
    echo "Check $CHECKS, Balance: $BALANCE, Expected: $EXPECTED_BALANCE"
    if [ "$BALANCE" == "$EXPECTED_BALANCE" ]; then
      echo "Bridge Out Sequencer success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Bridge Out Sequencer failure"
    exit 1
  fi

  echo "Testing TX Finalization"
  CHECKS=0
  BLOCK_NUM_HEX=$(just evm-get-transaction-receipt {{bridge_tx_hash}} | jq -r '.blockNumber')
  BLOCK_NUM=$(just hex-to-dec $BLOCK_NUM_HEX)
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    FINALIZED_BLOCK=$(just evm-get-block-by-number finalized | jq -r '.number')
    FINALIZED_BLOCK=$(just hex-to-dec $FINALIZED_BLOCK)
    echo "Check $CHECKS, Finalized block: $FINALIZED_BLOCK, Expected: $BLOCK_NUM"
    if [ $FINALIZED_BLOCK -ge $BLOCK_NUM ]; then
      echo "Finalization success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Finalization failure"
    exit 1
  fi
  exit 0

delete-smoke-test:
  just delete celestia-local
  just delete sequencer
  just delete rollup

evm_contract_address := "0xA58639fB5458e65E4fA917FF951C390292C24A15"
run-smoke-cli tag=defaultTag:
  #!/usr/bin/env bash
  set -e

  ASTRIA_CLI_IMAGE="{{cli_image}}{{ if tag != '' { replace(':#', '#', tag) } else { '' } }}"
  MAX_CHECKS=30

  # Checking starting balance
  BALANCE=$(just evm-get-balance {{evm_destination_address}})
  if [ $BALANCE -ne 0 ]; then
    echo "Starting balance is not correct"
    exit 1
  fi

  echo "Testing Bridge In..."
  just init rollup-bridge {{tag}}
  CHECKS=0
  EXPECTED_BALANCE=$(echo "{{sequencer_transfer_amount}} * {{sequencer_base_amount}} * {{rollup_multiplier}}" | bc)
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    BALANCE=$(just evm-get-balance {{evm_destination_address}})
    echo "Check $CHECKS, Balance: $BALANCE, Expected: $EXPECTED_BALANCE"
    if [ "$BALANCE" == "$EXPECTED_BALANCE" ]; then
      echo "Bridge In success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Bridge In failure"
    exit 1
  fi

  echo "Testing Bridge Out..."
  just evm-send-raw-transaction {{bridge_tx_bytes}}
  TRANSFERED_BALANCE=$(echo "1 * {{sequencer_base_amount}} * {{rollup_multiplier}}" | bc)
  EXPECTED_BALANCE=$(echo "$EXPECTED_BALANCE - $TRANSFERED_BALANCE" | bc)
  CHECKS=0
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    BALANCE=$(just evm-get-balance {{evm_destination_address}})
    echo "Check $CHECKS, Balance: $BALANCE, Expected: $EXPECTED_BALANCE"
    if [ "$BALANCE" == "$EXPECTED_BALANCE" ]; then
      echo "Bridge Out EVM success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Bridge Out EVM failure"
    exit 1
  fi

  CURRENT_BLOCK_HEX=$(just evm-get-block-by-number latest | jq -r '.number')
  CURRENT_BLOCK=$(just hex-to-dec $CURRENT_BLOCK_HEX)

  # Using a docker volume to handle both passing in a private key & the output file
  docker volume create cli-test-withdrawals
  docker run --rm -v cli-test-withdrawals:/data alpine sh -c "echo '{{sequencer_bridge_pkey}}' > /data/key"
  docker run --rm -v cli-test-withdrawals:/astria --network host $ASTRIA_CLI_IMAGE bridge collect-withdrawals \
    --rollup-endpoint {{eth_ws_url}} \
    --contract-address {{evm_contract_address}} \
    --from-rollup-height 1 \
    --to-rollup-height $CURRENT_BLOCK \
    --sequencer-asset-to-withdraw nria \
    --bridge-address {{sequencer_bridge_address}} \
    --output "/astria/tempfile" \
    --force
   docker run --rm -v cli-test-withdrawals:/astria --network host $ASTRIA_CLI_IMAGE bridge submit-withdrawals \
    --signing-key "/astria/key"  \
    --sequencer-chain-id {{sequencer_chain_id}} \
    --sequencer-url {{sequencer_rpc_url}} \
    --input "/astria/tempfile"

  # Can inspect the file by removing and looking in volume
  docker volume remove cli-test-withdrawals

  CHECKS=0
  EXPECTED_BALANCE=$(echo "1 * {{sequencer_base_amount}}" | bc)
  while (( $CHECKS < $MAX_CHECKS )); do
    CHECKS=$((CHECKS+1))
    BALANCE=$(docker run --rm --network host $ASTRIA_CLI_IMAGE sequencer account balance astria17w0adeg64ky0daxwd2ugyuneellmjgnxl39504 --sequencer-url {{sequencer_rpc_url}}  | awk '/nria/{print $(NF-1)}')
    echo "Check $CHECKS, Balance: $BALANCE, Expected: $EXPECTED_BALANCE"
    if [ "$BALANCE" == "$EXPECTED_BALANCE" ]; then
      echo "Bridge Out Sequencer success"
      break
    else
      sleep 1
    fi
  done
  if (( $CHECKS >= $MAX_CHECKS )); then
    echo "Bridge Out Sequencer failure"
    exit 1
  fi

  exit 0

#############################################
##    EVM Curl Command Helper Functions    ##
#############################################

evm-get-deployed-contract-code contractAddress rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"eth_getCode","params":["{{contractAddress}}", "latest"],"id":1}' -H 'Content-Type: application/json' | jq -r '.result'

evm-get-transaction-receipt txHash rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["{{txHash}}"],"id":1}' -H 'Content-Type: application/json' | jq '.result'

evm-get-raw-transaction txHash rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"eth_getRawTransactionByHash","params":["{{txHash}}"],"id":1}' -H 'Content-Type: application/json' | jq '.result'

evm-send-raw-transaction txData rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["{{txData}}"],"id":1}' -H 'Content-Type: application/json'

evm-get-block-by-number blockNumber rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["{{blockNumber}}", false],"id":1}' -H 'Content-Type: application/json' | jq '.result'

evm-get-balance address rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  HEX_NUM=$(curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"eth_getBalance","params":["{{address}}", "latest"],"id":1}' -H 'Content-Type: application/json' | jq -r '.result')
  echo $(just hex-to-dec $HEX_NUM)

evm-get-txpool-length rpcUrl=eth_rpc_url:
  #!/usr/bin/env bash
  curl -X POST {{eth_rpc_url}} -s -d '{"jsonrpc":"2.0","method":"txpool_content","params":[],"id":1}' -H 'Content-Type: application/json' | jq '.result'

hex-to-dec hex:
  #!/usr/bin/env bash
  HEX_NUM={{hex}}
  CLEAN_HEX=$(echo "${HEX_NUM#0x}" | tr 'a-z' 'A-Z')
  echo $(echo "ibase=16; $CLEAN_HEX" | bc)
