import random
import requests
from requests.adapters import HTTPAdapter, Retry
import subprocess

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
    raise RuntimeError(f"Failed to forward to a local port for {node_name}")

def check_change_infos(change_infos, expected_activation_height, expected_app_version=None):
    """Assert that the provided change info collection is not empty, and that each entry has the
    expected activation height and app version.

    Exits the process on failure.
    """
    if len(list(change_infos)) == 0:
        raise SystemExit("Sequencer upgrade error: no upgrade change info reported")
    for change_info in change_infos:
        if change_info.activation_height != expected_activation_height:
            raise SystemExit(
                "Sequencer upgrade error: reported change info does not have expected activation "
                f"height of {expected_activation_height}. Reported change info:\n{change_info}"
            )
        if expected_app_version and change_info.app_version != expected_app_version:
            raise SystemExit(
                "Sequencer upgrade error: reported change info does not have expected app version "
                f"of {expected_app_version}. Reported change info:\n{change_info}"
            )
