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

    def try_get_balance(self, account):
        """
        Tries to get the given account's balance by calling `astria-cli sequencer account balance`.
        """
        stdout = self._try_exec_sequencer_command_with_retry("account", "balance", account)
        balance_line = stdout.splitlines().pop()
        if balance_line.endswith("nri"):
            return int(balance_line[:-4])
        else:
            raise RuntimeError(
                "expected last line of cli `sequencer account balance` output to end with `nria`"
            )

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

    def _try_exec_sequencer_command_with_retry(self, *args, sequencer_name="node0", retries=1):
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
