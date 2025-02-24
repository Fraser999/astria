# Generated by the gRPC Python protocol compiler plugin. DO NOT EDIT!
"""Client and server classes corresponding to protobuf-defined services."""
import grpc
import warnings

from helpers.proto.generated import service_pb2 as helpers_dot_proto_dot_generated_dot_service__pb2

GRPC_GENERATED_VERSION = '1.70.0'
GRPC_VERSION = grpc.__version__
_version_not_supported = False

try:
    from grpc._utilities import first_version_is_lower
    _version_not_supported = first_version_is_lower(GRPC_VERSION, GRPC_GENERATED_VERSION)
except ImportError:
    _version_not_supported = True

if _version_not_supported:
    raise RuntimeError(
        f'The grpc package installed is at version {GRPC_VERSION},'
        + f' but the generated code in helpers/proto/generated/service_pb2_grpc.py depends on'
        + f' grpcio>={GRPC_GENERATED_VERSION}.'
        + f' Please upgrade your grpc module to grpcio>={GRPC_GENERATED_VERSION}'
        + f' or downgrade your generated code using grpcio-tools<={GRPC_VERSION}.'
    )


class SequencerServiceStub(object):
    """Missing associated documentation comment in .proto file."""

    def __init__(self, channel):
        """Constructor.

        Args:
            channel: A grpc.Channel.
        """
        self.GetUpgradesInfo = channel.unary_unary(
                '/astria.sequencerblock.v1.SequencerService/GetUpgradesInfo',
                request_serializer=helpers_dot_proto_dot_generated_dot_service__pb2.GetUpgradesInfoRequest.SerializeToString,
                response_deserializer=helpers_dot_proto_dot_generated_dot_service__pb2.GetUpgradesInfoResponse.FromString,
                _registered_method=True)


class SequencerServiceServicer(object):
    """Missing associated documentation comment in .proto file."""

    def GetUpgradesInfo(self, request, context):
        """Returns info about the sequencer upgrades applied and scheduled.
        """
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')


def add_SequencerServiceServicer_to_server(servicer, server):
    rpc_method_handlers = {
            'GetUpgradesInfo': grpc.unary_unary_rpc_method_handler(
                    servicer.GetUpgradesInfo,
                    request_deserializer=helpers_dot_proto_dot_generated_dot_service__pb2.GetUpgradesInfoRequest.FromString,
                    response_serializer=helpers_dot_proto_dot_generated_dot_service__pb2.GetUpgradesInfoResponse.SerializeToString,
            ),
    }
    generic_handler = grpc.method_handlers_generic_handler(
            'astria.sequencerblock.v1.SequencerService', rpc_method_handlers)
    server.add_generic_rpc_handlers((generic_handler,))
    server.add_registered_method_handlers('astria.sequencerblock.v1.SequencerService', rpc_method_handlers)


 # This class is part of an EXPERIMENTAL API.
class SequencerService(object):
    """Missing associated documentation comment in .proto file."""

    @staticmethod
    def GetUpgradesInfo(request,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.unary_unary(
            request,
            target,
            '/astria.sequencerblock.v1.SequencerService/GetUpgradesInfo',
            helpers_dot_proto_dot_generated_dot_service__pb2.GetUpgradesInfoRequest.SerializeToString,
            helpers_dot_proto_dot_generated_dot_service__pb2.GetUpgradesInfoResponse.FromString,
            options,
            channel_credentials,
            insecure,
            call_credentials,
            compression,
            wait_for_ready,
            timeout,
            metadata,
            _registered_method=True)
