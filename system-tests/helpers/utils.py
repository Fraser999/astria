import random
import requests
from requests.adapters import HTTPAdapter, Retry
import subprocess

def run_subprocess(args, msg):
    """
    Runs the provided args as a subprocess.

    `msg` will be printed along with the command being run, and also on failure of the subprocess.
    It should be of the form e.g. "upgrading node1".

    On error, exits the top-level process.
    """
    try:
        try_run_subprocess(args, msg)
    except RuntimeError as error:
        raise SystemExit(error)

def try_run_subprocess(args, msg):
    """
    Tries to run the provided args as a subprocess.

    `msg` will be printed along with the command being run. It should be of the form e.g.
    "upgrading node1".

    On error, raises a `RuntimeError` exception.
    """
    prefix = f"{msg}: " if msg else ""
    print(f"{prefix}running `{' '.join(map(str, args))}`")
    try:
        subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True)
    except subprocess.CalledProcessError as error:
        prefix = f" {msg}: " if msg else ": "
        raise RuntimeError(f"failed{prefix}{error.stdout.decode('utf-8').strip()}")

def wait_for_statefulset_rollout(deploy_name, statefulset_name, namespace, timeout_secs):
    args = [
        "kubectl", "rollout", "status", f"statefulset/{statefulset_name}", f"-n={namespace}",
        f"--timeout={timeout_secs}s"
    ]
    try:
        try_run_subprocess(args, f"waiting for {deploy_name} to deploy")
        return
    except RuntimeError as error:
        print(error)
    # Waiting failed.  Print potentially useful info.
    subprocess.run(["kubectl", "get", "pods", f"-n={namespace}"])
    print()
    subprocess.run(["kubectl", "events", f"-n={namespace}", "--types=Warning"])
    print()
    raise SystemExit(f"failed to deploy {deploy_name} within {timeout_secs} seconds")

def update_chart_dependencies(chart):
    args = ["helm", "dependency", "update", f"charts/{chart}"]
    run_subprocess(args, msg=f"updating chart dependencies for {chart}")

def start_port_forwarding(node_name, namespace, pod_port):
    """Start port-forwarding for the given node by running `kubectl port-forward` as a subprocess.

    Attempts to port-forward up to 10 times using a different random local port on each attempt.

    Returns the subprocess handle and the local port used on success or exits the process if all
    attempts fail.
    """
    max_attempts = 10
    attempts = 0
    while attempts < max_attempts:
        local_port = random.randint(1024, 65535)
        port_forward_process = subprocess.Popen(
            ["kubectl", "port-forward", f"-n={namespace}", "sequencer-0", f"{local_port}:{pod_port}"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        # If the port-forwarding succeeds and is valid, the subprocess will not terminate when a
        # client connection is attempted.
        #
        # Try a brief connect first...
        try:
            requests_session = requests.Session()
            retries = Retry(total=5, backoff_factor=0.1)
            requests_session.mount("http://", HTTPAdapter(max_retries=retries))
            requests_session.get(f"http://localhost:{local_port}")
        except:
            pass
        # ...then see if the subprocess has terminated, where timing out indicates success.
        try:
            _stdout, stderr = port_forward_process.communicate(timeout=0.5)
            output = (
                f"port-forwarding attempt for {node_name} pod port {pod_port} to local port "
                f"{local_port} failed:{f'\n{stderr.decode()}' if attempts + 1 == max_attempts else ' retrying\n'}"
            )
            print(output, end="")
        except subprocess.TimeoutExpired:
            print(f"port-forwarding {node_name} pod port {pod_port} to local port {local_port}")
            return port_forward_process, local_port
        attempts += 1
    raise RuntimeError(f"failed to forward to a local port for {node_name}")

def check_change_infos(change_infos, expected_activation_height, expected_app_version=None):
    """Assert that the provided change info collection is not empty, and that each entry has the
    expected activation height and app version.

    Exits the process on failure.
    """
    if len(list(change_infos)) == 0:
        raise SystemExit("sequencer upgrade error: no upgrade change info reported")
    for change_info in change_infos:
        if change_info.activation_height != expected_activation_height:
            raise SystemExit(
                "sequencer upgrade error: reported change info does not have expected activation "
                f"height of {expected_activation_height}: reported change info:\n{change_info}"
            )
        if expected_app_version and change_info.app_version != expected_app_version:
            raise SystemExit(
                "sequencer upgrade error: reported change info does not have expected app version "
                f"of {expected_app_version}: reported change info:\n{change_info}"
            )
