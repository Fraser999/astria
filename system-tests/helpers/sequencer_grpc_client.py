import grpc
import subprocess
import time
from helpers.proto.generated.service_pb2 import GetUpgradesInfoRequest
from helpers.proto.generated.service_pb2_grpc import SequencerServiceStub

# Queries the sequencer's gRPC server at <localhost:port> for the upgrades info.
#
# Exits the process on error or timeout.
def get_upgrades_info(grpc_port):
    # Need to port-forward the sequencer gRPC port as a background process.
    port_fwd_process = subprocess.Popen(
        [
            "kubectl", "port-forward", "-n=astria-dev-cluster",
            "service/node0-sequencer-grpc-service", f"{grpc_port}:{grpc_port}"
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    # Repeatedly try to execute the RPC until success or timeout.
    start = time.monotonic()
    timeout_duration = 5
    timeout = start + timeout_duration
    while True:
        time.sleep(0.1)
        try:
            response = _try_get_upgrades_info(grpc_port)
            port_fwd_process.terminate()
            return response.applied, response.scheduled
        except Exception as error:
            if time.monotonic() >= timeout:
                port_fwd_process.terminate()
                stdout, _stderr = port_fwd_process.communicate()
                raise SystemExit(
                    f"Failed to get upgrade info within {timeout_duration} seconds:\n{error}\n"
                    f"Output from port-forward attempt: `{stdout.decode()}`"
                )

def _try_get_upgrades_info(grpc_port):
    channel = grpc.insecure_channel(f"localhost:{grpc_port}")
    stub = SequencerServiceStub(channel)
    return stub.GetUpgradesInfo(GetUpgradesInfoRequest())
