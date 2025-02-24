The `service.proto` file in this folder was copied from
`proto/sequencerblockapis/astria/sequencerblock/v1/service.proto` and stripped down to simplify
executing the protoc compiler.

The files in the generated folder were created following instructions in
[the gRPC Python Basics tutorial](https://grpc.io/docs/languages/python/basics) using the following
commands:

```
pip install grpcio-tools
python -m grpc_tools.protoc \
  -I helpers/proto/generated=system-tests/helpers/proto \
  --python_out=system-tests \
  --pyi_out=system-tests \
  --grpc_python_out=system-tests \
  system-tests/helpers/proto/service.proto
```
