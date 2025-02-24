import requests

# Queries the sequencer's JSON-RPC server at `url` for the block at the specified height.
#
# Exits the process on error.
def get_block(url, height, verbose):
    try:
        return _send_json_rpc_request(url, "block", ("height", str(height)), verbose=verbose)
    except Exception as error:
        raise SystemExit(f"Failed to get block: {error}")

# Queries the sequencer's JSON-RPC server at `url` for the latest block height.
#
# Exits the process on error.
def get_last_block_height(url, verbose):
    try:
        return try_get_last_block_height(url, verbose)
    except Exception as error:
        raise SystemExit(f"Failed to get last block height: {error}")

# Queries the sequencer's JSON-RPC server at `url` for the latest block height.
#
# Throws a `requests` exception if the RPC call fails, or a `RuntimeError` if the JSON-RPC response
# is an error.
def try_get_last_block_height(url, verbose):
    response = _send_json_rpc_request(url, "abci_info", verbose=verbose)
    return int(response["response"]["last_block_height"])

# Queries the sequencer's JSON-RPC server at `url` for the app version as reported via the `genesis`
# method.
#
# Exits the process on error.
def get_app_version_at_genesis(url, verbose):
    try:
        response = _send_json_rpc_request(url, "genesis", verbose=verbose)
        return int(response["genesis"]["consensus_params"]["version"]["app"])
    except Exception as error:
        raise SystemExit(f"Failed to get current app version: {error}")

# Queries the sequencer's JSON-RPC server at `url` for the current app version as reported via the
# `abci_info` method.
#
# Exits the process on error.
def get_current_app_version(url, verbose):
    try:
        response = _send_json_rpc_request(url, "abci_info", verbose=verbose)
        return int(response["response"]["app_version"])
    except Exception as error:
        raise SystemExit(f"Failed to get current app version: {error}")

# Sends a JSON-RPC request to `url` with the given method and params.
#
# `params` should be pairs of key-value strings.
#
# Throws a `requests` exception if the RPC call fails, or a `RuntimeError` if the JSON-RPC response
# is an error.
def _send_json_rpc_request(url, method, *params, verbose):
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": dict(params),
        "id": 1,
    }

    try:
        response = requests.post(url, json=payload).json()
    except Exception as error:
        if verbose:
            print("Request:", payload)
        raise error

    if not "result" in response:
        if verbose:
            print("Request:", payload)
        raise RuntimeError(f"JSON-RPC error response for `{method}`: {response['error']}")

    return response["result"]
