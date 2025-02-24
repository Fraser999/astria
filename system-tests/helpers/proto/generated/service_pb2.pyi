from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Iterable as _Iterable, Mapping as _Mapping, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class GetUpgradesInfoRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetUpgradesInfoResponse(_message.Message):
    __slots__ = ("applied", "scheduled")
    class ChangeInfo(_message.Message):
        __slots__ = ("activation_height", "change_name", "app_version", "base64_hash")
        ACTIVATION_HEIGHT_FIELD_NUMBER: _ClassVar[int]
        CHANGE_NAME_FIELD_NUMBER: _ClassVar[int]
        APP_VERSION_FIELD_NUMBER: _ClassVar[int]
        BASE64_HASH_FIELD_NUMBER: _ClassVar[int]
        activation_height: int
        change_name: str
        app_version: int
        base64_hash: str
        def __init__(self, activation_height: _Optional[int] = ..., change_name: _Optional[str] = ..., app_version: _Optional[int] = ..., base64_hash: _Optional[str] = ...) -> None: ...
    APPLIED_FIELD_NUMBER: _ClassVar[int]
    SCHEDULED_FIELD_NUMBER: _ClassVar[int]
    applied: _containers.RepeatedCompositeFieldContainer[GetUpgradesInfoResponse.ChangeInfo]
    scheduled: _containers.RepeatedCompositeFieldContainer[GetUpgradesInfoResponse.ChangeInfo]
    def __init__(self, applied: _Optional[_Iterable[_Union[GetUpgradesInfoResponse.ChangeInfo, _Mapping]]] = ..., scheduled: _Optional[_Iterable[_Union[GetUpgradesInfoResponse.ChangeInfo, _Mapping]]] = ...) -> None: ...
