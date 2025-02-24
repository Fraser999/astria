import grpc
import requests
import subprocess
import time
from .utils import (
    check_change_infos,
    run_subprocess,
    start_port_forwarding,
    wait_for_statefulset_rollout,
)
from .proto.generated.service_pb2 import GetSequencerBlockRequest, GetUpgradesInfoRequest
from .proto.generated.service_pb2_grpc import SequencerServiceStub

RPC_POD_PORT = 26657
GRPC_POD_PORT = 8080

class SequencerController:
    """
    A controller targeting a single sequencer node.

    It provides methods for starting and upgrading the node and accessing the node's RPC and gRPC
    servers.
    """

    def __init__(self, node_name):
        self.name = node_name
        if node_name == "node0":
            namespace = "astria-dev-cluster"
        else:
            namespace = f"astria-validator-{node_name}"
        self.namespace = namespace
        self.rpc_port_forward_process = None
        self.rpc_local_port = None
        self.grpc_port_forward_process = None
        self.last_block_height_before_restart = None

    def __del__(self):
        self._terminate_port_forwarding()

    # ===========================================================
    # Methods managing and querying the sequencer's k8s container
    # ===========================================================

    def deploy_sequencer(
            self,
            image_tag,
            enable_price_feed=True,
            upgrade_name=None,
            upgrade_activation_height=None,
    ):
        """
        Deploys a new sequencer on the cluster using the specified image tag.

        The sequencer (and associated sequencer-relayer) are installed via `helm install`, then
        when the rollout has completed, port-forwarding of the sequencer's RPC and gRPC endpoints
        begins.

        If `upgrade_name` is set, then the chart value `upgradeTest` will be set to true to ensure
        genesis.json and upgrades.json have appropriate values set for pre-upgrade.

        In this case, whether the actual upgrade details for this upgrade are included in
        upgrades.json depends upon setting `upgrade_activation_height`. None means they are omitted
        (used when starting nodes before the upgrade has activated).
        """
        args = self._helm_args(
            "install",
            image_tag,
            enable_price_feed,
            upgrade_name,
            upgrade_activation_height
        )
        run_subprocess(args, msg=f"deploying {self.name}")
        self._wait_for_deploy(timeout_secs=600)
        self._ensure_reported_name_matches_assigned_name()

    def stage_upgrade(self, image_tag, enable_price_feed, upgrade_name, activation_height):
        """
        Updates the sequencer and sequencer-relayer in the cluster.

        This method simply stages the upgrade; it doesn't wait for the binaries to restart.
        """
        try:
            # If port-forwarding has already stopped, this is likely due to the sequencer having
            # crashed after missing the upgrade activation.  Don't try and restart port-forwarding
            # in this case, just set `last_block_height_before_restart` to `None`.
            if self.rpc_port_forward_process:
                self.last_block_height_before_restart = self.try_get_last_block_height()
            else:
                raise RuntimeError(f"{self.name} port-forwarding process stopped")
        except:
            self.last_block_height_before_restart = None

        # Update the upgrades.json file with the specified activation height and upgrade the images
        # for sequencer and sequencer-relayer.
        args = self._helm_args(
            "upgrade",
            image_tag,
            enable_price_feed=enable_price_feed,
            upgrade_name=upgrade_name,
            upgrade_activation_height=activation_height
        )

        run_subprocess(args, msg=f"upgrading {self.name}")
        if not self.last_block_height_before_restart:
            # In this case, the sequencer process has stopped. Try restarting the pod.
            run_subprocess(
                ["kubectl", "delete", "pod", f"-n={self.namespace}", "sequencer-0"],
                msg=f"restarting pod for {self.name}"
            )

    def wait_for_upgrade(self, upgrade_activation_height):
        """
        Waits for the sequencer to start following staging an upgrade and for it to execute the
        upgrade.

        Expected to be called after calling `stage_upgrade`.

        Note that restarting requires port-forwarding the sequencer's RPC and gRPC endpoints again,
        yielding (likely) different local ports from pre-upgrade.
        """
        # Allow 30s for termination, and a further 10s for deployment.
        self._wait_for_deploy(timeout_secs=40)

        # Wait for the sequencer to restart and commit two blocks after the last block recorded
        # before restarting.
        # NOTE: Two blocks rather than just one in case a new block was added in the small window
        #       between fetching the latest block height and actually shutting down.
        # NOTE: If `last_block_height_before_restart` is `None`, this node crashed rather than
        #       being killed for upgrade. This would happen if e.g. the node's binary wasn't
        #       replaced before the upgrade activation point. In this case, just skip the checks
        #       for scheduled upgrade change infos.
        if self.last_block_height_before_restart:
            self.wait_until_chain_at_height(
                self.last_block_height_before_restart + 2,
                timeout_secs=30
            )
            # Fetch and check the upgrade change infos. Ensure we're at least a few blocks before
            # the upgrade activation point, so we can safely expect there should be some changes
            # scheduled and none applied.
            latest_block_height = self.get_last_block_height()
            if latest_block_height < upgrade_activation_height - 2:
                applied, scheduled = self.get_upgrades_info()
                if len(list(applied)) != 0:
                    raise SystemExit(
                        f"{self.name} upgrade error: should have 0 applied upgrade change infos"
                    )
                check_change_infos(scheduled, upgrade_activation_height)
                for change_info in scheduled:
                    print(
                        f"{self.name}: scheduled change info: [{change_info.change_name}, "
                        f"activation_height: {change_info.activation_height}, app_version: "
                        f"{change_info.app_version}, change_hash: {change_info.base64_hash}]",
                        flush=True
                    )

        # Wait for the sequencer to reach the activation point, meaning it should have executed
        # the upgrade.
        latest_block_height = self.get_last_block_height()
        timeout_secs = max(upgrade_activation_height - latest_block_height, 1) * 10
        self.wait_until_chain_at_height(upgrade_activation_height, timeout_secs)

    # ===========================================
    # Methods calling sequencer's JSON-RPC server
    # ===========================================

    def get_last_block_height(self):
        """
        Queries the sequencer's JSON-RPC server for the latest block height.

        Exits the process on error.
        """
        try:
            response = self._try_send_json_rpc_request_with_retry("abci_info")
            return int(response["response"]["last_block_height"])
        except Exception as error:
            raise SystemExit(f"{self.name}: failed to get last block height: {error}")

    def try_get_last_block_height(self):
        """
        Tries once only to query the sequencer's JSON-RPC server for the latest block height.

        Throws a `requests` exception on error.
        """
        response = self._try_send_json_rpc_request("abci_info")
        return int(response["response"]["last_block_height"])

    def get_vote_extensions_enable_height(self):
        """
        Queries the sequencer's JSON-RPC server for `vote_extensions_enable_height` ABCI consensus
        parameter.

        Exits the process on error.
        """
        # NOTE: This RPC is flaky when no height is specified and often responds with e.g.
        # `{'code': -32603, 'message': 'Internal error', 'data': 'could not find consensus params
        # for height #123: value retrieved from db is empty'}`. Get the latest block height to pass
        # as an arg.
        height = self.get_last_block_height()
        response = self._try_send_json_rpc_request_with_retry(
            "consensus_params", ("height", str(height))
        )
        return int(response["consensus_params"]["abci"]["vote_extensions_enable_height"])

    def wait_until_chain_at_height(self, height, timeout_secs):
        """
        Polls the sequencer's JSON-RPC server for the latest block height until the given height is
        reached or exceeded.

        Exits the process if this condition is not achieved within `timeout_secs` seconds.
        """
        start = time.monotonic()
        timeout_instant = start + timeout_secs
        latest_block_height = None
        while True:
            try:
                latest_block_height = self.try_get_last_block_height()
            except Exception as error:
                print(
                    f"{self.name}: failed to get latest block height: {error}\n{self.name}: "
                    "retrying",
                    flush=True
                )
                pass
            now = time.monotonic()
            if latest_block_height and latest_block_height >= height:
                break
            if now >= timeout_instant:
                raise SystemExit(
                    f"{self.name} failed to reach block {height} within {timeout_secs} "
                    f"seconds. Latest block height: {latest_block_height}"
                )
            print(
                f"{self.name}: latest block height: {latest_block_height}, awaiting block {height}"
                f", {timeout_instant - now:.3f} seconds remaining",
                flush=True
            )
            time.sleep(1)
        print(f"{self.name}: latest block height: {latest_block_height}, finished awaiting block {height}")

    def get_app_version_at_genesis(self):
        """
        Queries the sequencer's JSON-RPC server for the app version as reported via the `genesis`
        method.

        Exits the process on error.
        """
        try:
            response = self._try_send_json_rpc_request_with_retry("genesis")
            return int(response["genesis"]["consensus_params"]["version"]["app"])
        except Exception as error:
            raise SystemExit(f"{self.name}: failed to get current app version: {error}")

    def get_current_app_version(self):
        """
        Queries the sequencer's JSON-RPC server for the current app version as reported via the
        `abci_info` method.

        Exits the process on error.
        """
        try:
            response = self._try_send_json_rpc_request_with_retry("abci_info")
            return int(response["response"]["app_version"])
        except Exception as error:
            raise SystemExit(f"{self.name}: failed to get current app version: {error}")

    # =======================================
    # Methods calling sequencer's gRPC server
    # =======================================

    def get_sequencer_block(self, height):
        """
        Queries the sequencer's gRPC server for the sequencer block at the given height.

        Exits the process on error or timeout.
        """
        try:
            return self._try_send_grpc_request_with_retry(GetSequencerBlockRequest(height=height))
        except Exception as error:
            raise SystemExit(f"{self.name}: failed to get sequencer block {height}:\n{error}\n")

    def get_upgrades_info(self):
        """
        Queries the sequencer's gRPC server for the upgrades info.

        Exits the process on error or timeout.
        """
        try:
            response = self._try_send_grpc_request_with_retry(GetUpgradesInfoRequest())
            return response.applied, response.scheduled
        except Exception as error:
            raise SystemExit(f"{self.name}: failed to get upgrade info:\n{error}\n")

    # ===============
    # Private methods
    # ===============

    def _helm_args(
            self,
            subcommand,
            image_tag,
            enable_price_feed,
            upgrade_name,
            upgrade_activation_height,
    ):
        args = [
            "helm",
            subcommand,
            f"-n={self.namespace}",
            f"{self.name}-sequencer-chart",
            "charts/sequencer",
            "--values=dev/values/validators/all.yml",
            f"--values=dev/values/validators/{self.name}.yml",
            f"--set=images.sequencer.devTag={image_tag}",
            f"--set=sequencer-relayer.images.sequencerRelayer.devTag={image_tag}",
            f"--set=ports.cometbftRpc={RPC_POD_PORT}",
            f"--set=ports.sequencerGrpc={GRPC_POD_PORT}",
            f"--set=sequencer.priceFeed.enabled={enable_price_feed}",
            "--set=sequencer.abciUDS=false",
        ]
        if subcommand == "install":
            args.append("--create-namespace")
        if upgrade_name:
            # This is an upgrade test: set `upgradeTest` so as to provide an upgrades.json file
            # and genesis.json without upgraded configs.  Also enable persistent storage.
            args.append("--set=sequencer.upgrades.systemTest.enabled=true")
            args.append("--set=storage.enabled=true")
            args.append("--set=sequencer-relayer.storage.enabled=true")
            # If we know the activation height of the upgrade, add it to the relevant upgrade's
            # settings for inclusion in the upgrades.json file.
            if upgrade_activation_height:
                args.append("--set=sequencer.upgrades.systemTest.bootstrapping=false")
                args.append(
                    f"--set=sequencer.upgrades.{upgrade_name}.activationHeight={upgrade_activation_height}"
                )
            else:
                # Otherwise, if no activation height is provided, exclude the entire upgrade from
                # the upgrades.json file.
                args.append("--set=sequencer.upgrades.systemTest.bootstrapping=true")
                args.append(f"--set=sequencer.upgrades.{upgrade_name}.included=false")
        return args

    def _wait_for_deploy(self, timeout_secs):
        wait_for_statefulset_rollout(self.name, "sequencer", self.namespace, timeout_secs)

    def _start_port_forwarding(self):
        try:
            self.rpc_port_forward_process, self.rpc_local_port = (
                start_port_forwarding(self.name, self.namespace, RPC_POD_PORT))
            self.grpc_port_forward_process, grpc_local_port = (
                start_port_forwarding(self.name, self.namespace, GRPC_POD_PORT))
        except RuntimeError as error:
            raise SystemExit(error)
        # Open a gRPC client connection.
        channel = grpc.insecure_channel(f"localhost:{grpc_local_port}")
        self.grpc_client = SequencerServiceStub(channel)

    def _terminate_port_forwarding(self):
        # If either of the port-forwarding processes have stopped, log their output.
        if self.rpc_port_forward_process:
            if self.rpc_port_forward_process.poll():
                _stdout, stderr = self.rpc_port_forward_process.communicate()
                print(
                    f"{self.name}: port-forwarding for RPC terminated:\n{stderr.decode()}"
                )
            else:
                self.rpc_port_forward_process.terminate()
        if self.grpc_port_forward_process:
            if self.grpc_port_forward_process.poll():
                _stdout, stderr = self.grpc_port_forward_process.communicate()
                print(
                    f"{self.name}: port-forwarding for gRPC terminated:\n{stderr.decode()}"
                )
            else:
                self.grpc_port_forward_process.terminate()
        self.rpc_port_forward_process = None
        self.grpc_port_forward_process = None

    def _try_send_json_rpc_request_with_retry(self, method, *params, retries=1):
        """
        Sends a JSON-RPC request to the associated sequencer's RPC server with the given method and
        params.

        `params` should be pairs of key-value strings.

        If the first attempt fails, retries the specified number of times, restarting the
        port-forward processes each time.

        Throws a `requests` exception if all the RPC calls fail, or a `RuntimeError` if a JSON-RPC
        response is an error.
        """
        attempts = 0
        while True:
            try:
                return self._try_send_json_rpc_request(method, *params)
            except Exception:
                if attempts == retries:
                    print(f"{self.name}: rpc failed {retries + 1} times - giving up")
                    raise
                else:
                    print(f"{self.name}: rpc failed - retrying")
                    attempts += 1

    def _try_send_json_rpc_request(self, method, *params, verbose=True):
        """
        Sends a single JSON-RPC request (i.e. no retries) to the associated sequencer's RPC
        server with the given method and params.

        `params` should be pairs of key-value strings.

        Throws a `requests` exception if the RPC call fails, or a `RuntimeError` if the JSON-RPC
        response is an error.
        """

        # Start port-forwarding if not already running.
        if not self.rpc_port_forward_process:
            self._start_port_forwarding()

        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": dict(params),
            "id": 1,
        }
        try:
            response = requests.post(f"http://localhost:{self.rpc_local_port}", json=payload).json()
        except Exception:
            if verbose:
                print(f"{self.name}: failed request: {payload}")
            self._terminate_port_forwarding()
            raise
        if not "result" in response:
            if verbose:
                print(f"{self.name}: failed request: {payload}")
            raise RuntimeError(f"json-rpc error response for `{method}`: {response['error']}")
        return response["result"]

    def _try_send_grpc_request_with_retry(self, request, retries=1):
        """
        Sends a gRPC request to the associated sequencer's gRPC server.

        If first attempt fails, retries the specified number of times, restarting the port-forward
        processes each time.

        Throws an exception if all the gRPC calls fail.
        """
        attempts = 0
        while True:
            try:
                return self._try_send_grpc_request(request)
            except Exception:
                if attempts == retries:
                    print(f"{self.name}: grpc failed {retries + 1} times - giving up")
                    raise
                else:
                    print(f"{self.name}: grpc failed - retrying")
                    attempts += 1

    def _try_send_grpc_request(self, request):
        """
        Sends a single gRPC request (i.e. no retries) to the associated sequencer's gRPC server.

        Throws an exception if the gRPC call fails.
        """

        # Start port-forwarding if not already running.
        if not self.rpc_port_forward_process:
            self._start_port_forwarding()

        try:
            if isinstance(request, GetSequencerBlockRequest):
                return self.grpc_client.GetSequencerBlock(request)
            elif isinstance(request, GetUpgradesInfoRequest):
                return self.grpc_client.GetUpgradesInfo(request)
            else:
                raise SystemExit(
                    f"{self.name}: failed to send gRPC request: {request} is an unknown type"
                )
        except Exception:
            print(f"{self.name}: request: {request}")
            self._terminate_port_forwarding()
            raise

    def _ensure_reported_name_matches_assigned_name(self):
        """
        Ensures the node name provided in `__init__` matches the moniker of the node we're
        associated with.
        """
        try:
            response = self._try_send_json_rpc_request_with_retry("status", retries=5)
            reported_name = response["node_info"]["moniker"]
            if reported_name == self.name:
                return
            else:
                raise SystemExit(
                    f"provided name `{self.name}` does not match moniker `{reported_name}` as "
                    "reported in `status` json-rpc response"
                )
        except Exception as error:
            raise SystemExit(
                f"{self.name}: failed to fetch node name: {error}"
            )
