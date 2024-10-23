# ed25519 threshold signing

## Usage

For DKG and signing, participants must have a secure channel between each other
 for communicating required messages.

### Distributed key generation

Each participant must choose an index from `[1..max_signers]`. Every
participant must use the same `min-signers` and `max-signers` parameters.

There are two communication rounds for the DKG protocol.

Eg. for participant with index 1:

```sh
cargo run -- sequencer threshold dkg --index 1 --min-signers 2 --max-signers 3 \
 --secret-key-package-path frost_1.priv --public-key-package-path frost.pub
```

This will open an interactive session where you will be prompted for information
from the other participants.

Expected output:

```sh
DKG completed successfully!
Secret key package saved to: frost_1.priv
Public key package saved to: frost.pub
```

The public key file will show the `verifying_key` generated by the protocol. Eg:

```sh
{
  "header": {
    "version": 0,
    "ciphersuite": "FROST-ED25519-SHA512-v1"
  },
  "verifying_shares": {
    "0100000000000000000000000000000000000000000000000000000000000000": "6a75f19a4e0477b3de6bee6408170b2aced0605a81ad011603606bf5ea6bc9e1",
    "0200000000000000000000000000000000000000000000000000000000000000": "068b5ff30b2fa6033b5e6577f7befffb2a237e37f38da8f4bbf3bc14955182ae"
  },
  "verifying_key": "e88f87ef4610d80753bdad1fa9289ef3858ae89fa28749d33a7f0ab1694c38e9"
}
```

Each participant generates the same public key file.

### Signing

Signing requires two stages from each participant and two stages from a
"coordinator". Each participant can act as their own coordinator, or one
party can act as the coordinator and transmit the outputs to the other
parties. `min-signers` must take part to produce a signature.

Note that the coordinator commands are interactive and require inputs
generated by the participants in the preceding steps.

1. Each participant runs part 1:

      ```sh
      cargo run -- sequencer threshold sign part1 \
      --secret-key-package-path frost_1.priv \
      --nonces-path nonces_1.out
      ```

1. The coordinator generates a signing package given the message
and the commitments from part 1.

      Note that by default, the message is expected to be a json-encoded
      `TransactionBody`, which is then re-encoded as a protobuf message.
      To sign a plaintext message, use the `--plaintext` flag.

      ```sh
      cargo run -- sequencer threshold sign prepare-message \
      --message-path msg.txt \
      --signing-package-path signing_package.out
      ```

1. Each participant runs part 2, given the signing package from the previous
step:

      ```sh
      cargo run -- sequencer threshold sign part2 \
      --secret-key-package-path frost_1.priv \
      --nonces-path nonces_1.out \
      --signing-package-path signing_package.out
      ```

1. The coordinator aggregates all the signature shares from the previous step:

      ```sh
      cargo run -- sequencer threshold sign aggregate \
      --public-key-package-path frost.pub \
      --signing-package-path signing_package.out
      ```

The output is an ed25519 signature, eg:

```sh
Aggregated signature: 6e999e7ce9a7a833af7503547baa2e90d6c69bc4a91dfec3390b438ece5a3c75706b44c97127b1f364d4f620ecba61dc0c1311e4e68ff288e6424185c826a80c
```

Optionally, if you specify the message that was signed, the CLI will output
the signature and message as a sequencer `SignedTransaction`. Eg:

```sh
cargo run -- sequencer threshold sign aggregate \
--public-key-package-path frost.pub \
--signing-package-path signing_package.out \
--message-path msg.txt \
--output-path transaction.out
```

### Verification

To verify a signature:

```sh
cargo run -- sequencer threshold verify \
--verifying-key <HEX-VERIFYING-KEY> \
--message-path msg.txt \
--signature <HEX-SIGNATURE>
```