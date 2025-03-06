import requests

helm install -n astria-dev-cluster astria-chain-chart ./charts/evm-stack -f dev/values/rollup/dev.yaml \
--set evm-rollup.images.conductor.devTag={{tag}} \
--set composer.images.composer.devTag={{tag}} \
--set evm-bridge-withdrawer.images.evmBridgeWithdrawer.devTag={{tag}} \
--set blockscout-stack.enabled=false \
--set postgresql.enabled=false \
--set evm-faucet.enabled=false > /dev/null
@just wait-for-rollup > /dev/null

def get_balance(url, address):
    hex_balance = json_rpc(url, "eth_getBalance", address, "latest")
    return int(hex_balance, 16)

def json_rpc(url, method, *params):
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": list(params),
        "id": 1,
    }
    response = requests.post(url, json=payload).json()
    if "result" in response:
        return response["result"]
    raise RuntimeError(f"JSON-RPC error response for `{method}`: {response['error']}")

balance = get_balance("https://rpc.flame.astria.org", "0xF30aA4F9AdEcb8bB209F764D300CbF78341d5e55")
print(balance)
