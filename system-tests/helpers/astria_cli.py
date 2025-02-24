import time
from python_on_whales import docker
from python_on_whales.exceptions import DockerException
from .utils import start_port_forwarding

SEQUENCER_RPC_POD_PORT = 26657

class Cli:
    """
    An instance of the astria-cli.
    """

    def __init__(self, image_tag="latest"):
        self.image_tag = image_tag
        self.sequencer_name = "node0"
        self.sequencer_rpc_port_forward_process = None
        self.sequencer_rpc_local_port = None

    def __del__(self):
        self._terminate_port_forwarding()

    def set_image_tag(self, image_tag):
        self.image_tag = image_tag

    def wait_until_balance(self, account, expected_balance, timeout_secs, sequencer_name):
        """
        Polls for the balance of the given account until the expected balance is reached.

        Exits the process if this condition is not achieved within `timeout_secs` seconds.
        """
        start = time.monotonic()
        timeout_instant = start + timeout_secs
        balance = None
        while True:
            try:
                balance = self._try_get_balance(account, sequencer_name)
                if balance == expected_balance:
                    break
            except Exception as error:
                print(f"failed to get balance: {error}")
                pass
            now = time.monotonic()
            if now >= timeout_instant:
                raise SystemExit(
                    f"failed to get balance {expected_balance} within {timeout_secs} "
                    f"seconds. Current evm balance: {balance}"
                )
            print(
                f"current balance: {balance}, awaiting balance of {expected_balance}, "
                f"{timeout_instant - now:.3f} seconds remaining"
            )
            time.sleep(1)
        print(f"current balance: {balance}, finished waiting")

    def init_bridge_account(self):
        try:
            self._try_exec_sequencer_command_with_retry(
                "init-bridge-account",
                "--rollup-name=astria",
                "--private-key=dfa7108e38ab71f89f356c72afc38600d5758f11a8c337164713e4471411d2e0",
                "--sequencer.chain-id=sequencer-test-chain-0",
                "--fee-asset=nria",
                "--asset=nria",
            )
        except Exception as error:
            raise SystemExit(error)

    def bridge_lock(self):
        try:
            self._try_exec_sequencer_command_with_retry(
                "bridge-lock",
                "astria13ahqz4pjqfmynk9ylrqv4fwe4957x2p0h5782u",
                "--amount=10000000000",
                "--destination-chain-address=0xaC21B97d35Bf75A7dAb16f35b111a50e78A72F30",
                "--private-key=934ab488f9e1900f6a08f50605ce1409ca9d95ebdc400dafc2e8a4306419fd52",
                "--sequencer.chain-id=sequencer-test-chain-0",
                "--fee-asset=nria",
                "--asset=nria",
            )
        except Exception as error:
            raise SystemExit(error)

    def _start_port_forwarding(self):
        print(f"cli: starting port-forwarding to {self.sequencer_name}")
        try:
            if self.sequencer_name == "node0":
                namespace = "astria-dev-cluster"
            else:
                namespace = f"astria-validator-{self.sequencer_name}"
            self.sequencer_rpc_port_forward_process, self.sequencer_rpc_local_port = (
                    start_port_forwarding(self.sequencer_name, namespace, SEQUENCER_RPC_POD_PORT))
        except RuntimeError as error:
            raise SystemExit(error)

    def _terminate_port_forwarding(self):
        # If the port-forwarding process has stopped, log its output.
        if self.sequencer_rpc_port_forward_process:
            if self.sequencer_rpc_port_forward_process.poll():
                _stdout, stderr = self.sequencer_rpc_port_forward_process.communicate()
                print(
                    f"cli: port-forwarding to {self.sequencer_name} terminated:\n{stderr.decode()}"
                )
            else:
                self.sequencer_rpc_port_forward_process.terminate()
        self.sequencer_rpc_port_forward_process = None

    def _try_get_balance(self, account, sequencer_name):
        """
        Tries to get the given account's balance by calling `astria-cli sequencer account balance`.
        """
        stdout = self._try_exec_sequencer_command_with_retry(
            "account", "balance", account, sequencer_name=sequencer_name
        )
        balance_line = stdout.splitlines().pop()
        if balance_line.endswith("nria"):
            return int(balance_line[:-4])
        else:
            raise RuntimeError(
                "expected last line of cli `sequencer account balance` output to end with `nria`: "
                f"stdout: `{stdout}`"
            )

    def _try_exec_sequencer_command_with_retry(self, *args, sequencer_name="node0", retries=9):
        """
        Tries to execute the CLI `sequencer` subcommand via `docker run`.

        `sequencer` and `--sequencer-url` should NOT be passed in the `args`; they will be added in
        this method based upon the value of `sequencer_name`.

        If the first attempt fails, retries the specified number of times, restarting the
        port-forward process each time.
        """
        attempts = 0
        while True:
            try:
                return self._try_exec_sequencer_command(*args, sequencer_name=sequencer_name)
            except Exception as error:
                if attempts == retries:
                    print(f"cli: execution failed {retries + 1} times - giving up")
                    raise error
                else:
                    print(f"cli: execution failed - retrying")
                    attempts += 1

    def _try_exec_sequencer_command(self, *args, sequencer_name="node0"):
        """
        Tries once (i.e. no retries) to execute the CLI `sequencer` subcommand via `docker run`.

        `sequencer` and `--sequencer-url` should NOT be passed in the `args`; they will be added in
        this method based upon the value of `sequencer_name`.

        Returns the stdout output on success, or throws a `DockerException` otherwise.
        """

        # Start port-forwarding if not already running or targeting a different sequencer.
        if not self.sequencer_rpc_port_forward_process or self.sequencer_name != sequencer_name:
            self.sequencer_name = sequencer_name
            self._start_port_forwarding()

        args = list(args)
        args.insert(0, "sequencer")
        args.append(f"--sequencer-url=http://localhost:{self.sequencer_rpc_local_port}")
        print(
            f"cli: running `docker run --rm --network "
            f"host ghcr.io/astriaorg/astria-cli:{self.image_tag} {' '.join(map(str, args))}`"
        )
        try:
            return docker.run(
                f"ghcr.io/astriaorg/astria-cli:{self.image_tag}",
                args,
                networks=["host"],
                remove=True,
            )
        except DockerException as error:
            print(f"Exit code {error.return_code} while running {error.docker_command}")
            self._terminate_port_forwarding()
            raise error
