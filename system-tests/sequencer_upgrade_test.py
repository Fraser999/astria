"""
This script provides a general test to ensure logic common to all sequencer upgrades have executed
correctly.

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
import concurrent
import sequencer_upgrade_1_checks
from sequencer_controller import SequencerController
from helpers.utils import check_change_infos

# The number of sequencer validator nodes to use in the test.
NUM_NODES = 5
# A map of upgrade name to sequencer image to use for running BEFORE the given upgrade is executed.
PRE_UPGRADE_IMAGE_TAGS = {
    "upgrade1": "pr-1751"
}

parser = argparse.ArgumentParser(prog="upgrade_test", description="Runs the sequencer upgrade test.")
parser.add_argument(
    "-t", "--image-tag",
    help=
        "The tag specifying the sequencer image to run to execute the upgrade, e.g. 'latest', \
        'local', 'pr-2000'. NOTE: this is not the image used to run sequencers before the upgrade \
        is staged; that image is chosen based upon the provided --upgrade-name value.",
    metavar="TAG",
    required=True
)
parser.add_argument(
    "-n", "--upgrade-name",
    help="The name of the upgrade to apply.",
    choices=("upgrade1",),
    required=True
)
args = vars(parser.parse_args())
upgrade_image_tag = args["image_tag"]
upgrade_name = args["upgrade_name"].lower()

print("################################################################################")
print("Running sequencer upgrade test")
print(f"  * upgraded container image tag: {upgrade_image_tag}")
print(f"  * pre-upgrade container image tag: {PRE_UPGRADE_IMAGE_TAGS[upgrade_name]}")
print(f"  * upgrade name: {upgrade_name}")
print("################################################################################")

nodes = [SequencerController(f"node{i}") for i in range(NUM_NODES - 1)]
print(f"Starting {len(nodes)} sequencers")
for node in nodes:
    node.deploy_sequencer(
        PRE_UPGRADE_IMAGE_TAGS[upgrade_name],
        # Needed to persist storage across upgrade.
        enable_persistent_storage=True,
        upgrade_name=upgrade_name,
    )

# Note block 1 and the current app version before attempting the upgrade.
for node in nodes:
    node.wait_until_chain_at_height(1, 60)
block_1_before = nodes[0].get_sequencer_block(1)
app_version_before = nodes[0].get_current_app_version()
genesis_app_version = nodes[0].get_app_version_at_genesis()
if app_version_before != genesis_app_version:
    raise SystemExit(
        f"Expected genesis app version {genesis_app_version} to be the same as the current app "
        f"version {app_version_before}.\nPossibly this test has already run on this network, or "
        "persistent volume data has not been deleted between attempts?\nTry running `just clean "
        "&& rm -r /tmp/astria` (sudo may be required for `rm -r /tmp/astria`) before re-running "
        "the test."
    )

# Ensure all other sequencers report the same values.
for node in nodes[1:]:
    if block_1_before != node.get_sequencer_block(1):
        raise SystemExit(f"node0 and {node.name} report different values for block 1")
    if app_version_before != node.get_current_app_version():
        raise SystemExit(f"node0 and {node.name} report different values for current app version")
    if genesis_app_version != node.get_app_version_at_genesis():
        raise SystemExit(f"node0 and {node.name} report different values for genesis app version")

# Run pre-upgrade checks specific to this upgrade.
print(f"Running pre-upgrade checks specific to {upgrade_name}")
if upgrade_name == "upgrade1":
    sequencer_upgrade_1_checks.assert_pre_upgrade_conditions(nodes)
print(f"Passed {upgrade_name}-specific pre-upgrade checks")

print("App version before upgrade:", app_version_before)

# Get the current block height from the sequencer and set the upgrade to activate soon.
block_height_difference = 10
latest_block_height = nodes[0].get_last_block_height()
upgrade_activation_height = latest_block_height + block_height_difference
print("Setting upgrade activation height to", upgrade_activation_height)
# Leave the last sequencer running the old binary through the upgrade to ensure it can catch up
# later. Pop it from the `nodes` list and re-add it later once it's caught up.
missed_upgrade_node = nodes.pop()
print(f"Not upgrading {missed_upgrade_node.name} until the rest have executed the upgrade")
for node in nodes:
    node.stage_upgrade(upgrade_image_tag, upgrade_name, upgrade_activation_height)

# Wait for the rollout to complete.
print("Waiting for pods to become ready")
executor = concurrent.futures.ThreadPoolExecutor(5)
wait_for_upgrade_fn = lambda seq_node: seq_node.wait_for_upgrade(upgrade_activation_height)
futures = [executor.submit(wait_for_upgrade_fn, node) for node in nodes]
concurrent.futures.wait(futures)

# Ensure the last sequencer has stopped.
try:
    if missed_upgrade_node.try_get_last_block_height() >= upgrade_activation_height:
        raise SystemExit(f"{missed_upgrade_node.name} should be stalled but isn't")
except Exception as error:
    # This is the expected branch - the node should have crashed when it disagreed about the outcome
    # of executing the block at the upgrade activation height.
    pass
print(f"{missed_upgrade_node.name} lagging as expected; now upgrading")

# Now stage the upgrade on this lagging node and ensure it catches up.
missed_upgrade_node.stage_upgrade(upgrade_image_tag, upgrade_name, upgrade_activation_height)
missed_upgrade_node.wait_for_upgrade(upgrade_activation_height)
print(f"{missed_upgrade_node.name} has caught up")
# Re-add the lagging node to the list.
nodes.append(missed_upgrade_node)

# Start a fifth sequencer validator now that the upgrade has happened.
new_node = SequencerController(f"node{NUM_NODES - 1}")
print(f"Starting a new sequencer")
new_node.deploy_sequencer(
    upgrade_image_tag,
    enable_persistent_storage=True,
    upgrade_name=upgrade_name,
    upgrade_activation_height=upgrade_activation_height
)

# Wait for the new node to catch up and go through the upgrade too.
new_node.wait_until_chain_at_height(upgrade_activation_height + 2, 60)
print(f"New sequencer {new_node.name} has caught up")
# Add the new node to the list.
nodes.append(new_node)

# Check the app version has increased.
app_version_after = nodes[0].get_current_app_version()
for node in nodes[1:]:
    if node.get_current_app_version() <= app_version_before:
        raise SystemExit(f"{node.name} failed to upgrade. App version unchanged")
    if node.get_current_app_version() != app_version_after:
        raise SystemExit(f"node0 and {node.name} report different values for app version")
print("App version changed after upgrade to:", app_version_after)

# Check that fetching block 1 yields the same result as before the upgrade (ensures test network
# didn't just restart from genesis using the upgraded binary rather than actually performing a
# network upgrade).
block_1_after = nodes[0].get_sequencer_block(1)
if block_1_before != block_1_after:
    raise SystemExit(
        "node0 failed to upgrade. Block 1 is different as reported before and after the upgrade"
    )
for node in nodes[1:]:
    if node.get_sequencer_block(1) != block_1_after:
        raise SystemExit(f"node0 and {node.name} report different values for block 1")
print("Fetching block 1 after the upgrade yields the same result as before the upgrade")

# Fetch and check the upgrade change infos. There should be none scheduled and at least one applied.
applied, scheduled = nodes[0].get_upgrades_info()
if len(list(scheduled)) != 0:
    raise SystemExit("node0 upgrade error: should have no scheduled upgrade change infos")
check_change_infos(applied, upgrade_activation_height, app_version_after)
for node in nodes[1:]:
    this_applied, this_scheduled = node.get_upgrades_info()
    if this_applied != applied:
        raise SystemExit(
            f"node0 and {node.name} report different values for applied upgrade changes"
        )
    if this_scheduled != scheduled:
        raise SystemExit(
            f"node0 and {node.name} report different values for scheduled upgrade changes"
        )
print("Upgrade change infos reported correctly")

# Run post-upgrade checks specific to this upgrade.
print(f"Running post-upgrade checks specific to {upgrade_name}")
if upgrade_name == "upgrade1":
    sequencer_upgrade_1_checks.assert_post_upgrade_conditions(nodes, upgrade_activation_height)
print(f"Passed {upgrade_name}-specific post-upgrade checks")

print("Sequencer network upgraded successfully")
