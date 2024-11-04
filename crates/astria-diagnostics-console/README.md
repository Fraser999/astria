# `astria-diagnostics-console`

Example non-interactive usage:

```shell
socat STDIO UNIX-CLIENT:/tmp/.sequencer-diagnostics.socket < \
<(echo -e 'config set --show-outcome=f\ns v k' && sleep 30) \
> /tmp/all-keys-verifiable.txt
```
