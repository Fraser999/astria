"""
To run using the latest sequencer image:

    just deploy cluster
    just deploy upgrade-test
    just run-upgrade-test latest

To run using a local build:

    just deploy cluster
    cargo check
    just docker-build-and-load astria-sequencer
    just deploy upgrade-test
    just run-upgrade-test local

The final commands above invokes this script as:

    python system-tests/upgrade_test.py --tag local --sequencer-rpc-url http://rpc.sequencer.localdev.me

To get verbose output, append a further arg using any value to the `just` command:

    just run-upgrade-test local v
"""

import argparse
import subprocess
import sys
import time
from helpers.sequencer_grpc_client import get_upgrades_info
from helpers.sequencer_rpc_client import (
    get_app_version_at_genesis,
    get_block,
    get_current_app_version,
    get_last_block_height,
    try_get_last_block_height,
)
from helpers.utils import check_change_infos

parser = argparse.ArgumentParser(prog="upgrade_test", description="Runs the sequencer upgrade test.")
parser.add_argument(
    "-t", "--tag",
    help="The tag specifying the sequencer image to use.",
    required=True
)
parser.add_argument(
    "-u", "--sequencer-rpc-url",
    help="The URL of the sequencer's RPC endpoint.",
    metavar="URL",
    required=True
)
parser.add_argument(
    "-v", "--verbose",
    help="Print verbose output.",
    action="store_true"
)
args = vars(parser.parse_args())
tag = args["tag"]
sequencer_rpc_url = args["sequencer_rpc_url"]
verbose = args["verbose"]
# NOTE: This should probably be passed in as a command line arg, but it is already hard-coded in
# several places throughout the charts.
sequencer_grpc_port = 8080

# Note block 1 and the current app version before attempting the upgrade.
block_1_before = get_block(sequencer_rpc_url, 1, verbose)
app_version_before = get_current_app_version(sequencer_rpc_url, verbose)
genesis_app_version = get_app_version_at_genesis(sequencer_rpc_url, verbose)
if app_version_before != genesis_app_version:
    raise SystemExit(
        f"Expected genesis app version {genesis_app_version} to be the same as the current app "
        f"version {app_version_before}.\nPossibly this test has already run on this network, or "
        "persistent volume data has not been deleted between attempts?\nTry running `just clean "
        "&& rm -r /tmp/astria` (sudo may be required for `rm -r /tmp/astria`) before re-running "
        "the test."
    )

if verbose:
    print("App version before upgrade:", app_version_before)

# Get the current block height from the sequencer and set the upgrade to activate soon.
block_height_difference = 10
latest_block_height = get_last_block_height(sequencer_rpc_url, verbose)
upgrade_activation_height = latest_block_height + block_height_difference
if verbose:
    print("Setting upgrade activation height to", upgrade_activation_height)
configmap = subprocess.run(
    [
        "helm", "template", "charts/sequencer", "--dry-run",
        "--show-only=templates/upgrade_configmap.yaml",
        "--values=dev/values/validators/all.yml",
        "--values=dev/values/validators/single.yml",
        "--set=genesis.postLatestUpgrade=false",
        f"--set=sequencer.upgrades.upgrade1.activationHeight={upgrade_activation_height}"
    ],
    capture_output=True
).stdout
subprocess.run(
    ["kubectl", "apply", "-n=astria-dev-cluster", "--filename=-"],
    input=configmap,
    capture_output=(not verbose))

# Upgrade the image to use the new binary.
subprocess.run(
    [
        "kubectl", "set", "image", "-n=astria-dev-cluster",
        "statefulset", "sequencer", f"sequencer=ghcr.io/astriaorg/sequencer:{tag}"
    ],
    capture_output=(not verbose)
)

# Wait for the rollout to complete.  Allow 30s for termination, and a further 10s for deployment.
print("Waiting for sequencer to upgrade")
rollout_timeout = 40
status_check = subprocess.run(
    [
        "kubectl", "rollout", "status", "statefulset/sequencer",
        "-n=astria-dev-cluster", f"--timeout={rollout_timeout}s"
    ],
    capture_output=(not verbose)
)
if status_check.returncode != 0:
    subprocess.run(["kubectl", "get", "pods", "-n=astria-dev-cluster"], capture_output=False)
    print()
    subprocess.run(
        [
            "kubectl", "events", "-n=astria-dev-cluster", "--for=Pod/sequencer-0",
            "--types=Warning"
        ], capture_output=False)
    print()
    raise SystemExit(f"Failed to deploy the upgrade within {rollout_timeout} seconds")

# Wait for the sequencer to restart and reach the activation point.
start = time.monotonic()
timeout_duration = block_height_difference * 3
timeout = start + timeout_duration
printed_change_infos = False
while latest_block_height < upgrade_activation_height:
    time.sleep(1)
    if time.monotonic() >= timeout:
        if not verbose:
            print()
        raise SystemExit(
            f"Sequencer failed to upgrade within {timeout_duration} seconds. Latest block height: "
            f"{latest_block_height}, upgrade activation height: {upgrade_activation_height}"
        )
    try:
        latest_block_height = try_get_last_block_height(sequencer_rpc_url, verbose)
        # Since fetching the latest block succeeded, we should be able to fetch and check the
        # upgrade change infos. Ensure we're at least a few blocks before the upgrade activation
        # point, so we can safely expect there should be some changes scheduled and none applied.
        if latest_block_height < upgrade_activation_height - 2:
            applied, scheduled = get_upgrades_info(sequencer_grpc_port)
            if len(list(applied)) != 0:
                raise SystemExit("Sequencer upgrade error: should have no applied upgrade change infos")
            check_change_infos(scheduled, upgrade_activation_height)
            if verbose and not printed_change_infos:
                printed_change_infos = True
                for change_info in scheduled:
                    print(f"scheduled change info:\n{change_info}", end="")
    except Exception as error:
        if verbose:
            print(f"Failed getting latest block height: {error}")
            print("Retrying")
        pass
    if verbose:
        print(f"Latest block height: {latest_block_height}")
    else:
        print(".", end="")
    sys.stdout.flush()

if verbose:
    print(f"Latest block height: {latest_block_height}")
else:
    print()

# Check the app version has increased.
app_version_after = get_current_app_version(sequencer_rpc_url, verbose)
if app_version_after <= app_version_before:
    raise SystemExit("Sequencer failed to upgrade. App version unchanged")
if verbose:
    print("App version changed after upgrade to:", app_version_after)

# Check that fetching block 1 yields the same result as before the upgrade (ensures test network
# didn't just restart from genesis using the upgraded binary rather than actually performing a
# network upgrade).
block_1_after = get_block(sequencer_rpc_url, 1, verbose)
if block_1_before != block_1_after:
    raise SystemExit(
        "Sequencer failed to upgrade. Block 1 is different as reported before and after the upgrade"
    )
if verbose:
    print("Fetching block 1 after the upgrade yields the same result as before the upgrade")

# Fetch and check the upgrade change infos. There should be none scheduled and at least one applied.
applied, scheduled = get_upgrades_info(sequencer_grpc_port)
if len(list(scheduled)) != 0:
    raise SystemExit("Sequencer upgrade error: should have no scheduled upgrade change infos")
check_change_infos(applied, upgrade_activation_height, app_version_after)
if verbose:
    print("Upgrade change infos reported correctly")

print("Sequencer upgraded successfully")
