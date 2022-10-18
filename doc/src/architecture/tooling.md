# Tooling

## DarkFi Fullnode Daemon

`drkd` the darkfi fullnode. It manages the blockchain, validates transactions
and remains connected to the p2p network.

Clients can connect over localhost RPC or secure socket and perform these functions:

* Get the node status and modify settings realtime.
* Query the blockchain.
* Broadcast txs to the p2p network.
* Get tx status, query the mempool and interact with components.

`drkd` does not have any concept of keys or wallet functionality. It does not
manage keys.

## Low Level Client

Clients manage keys and objects. They make queries to `drkd`, and receive notes
encrypted to their public keys.

Their design is usually specific to their application but modular.

They also expose a high level simple to use API corresponding **exactly** to
their commands so that product teams can easily build an application. They will
use the command line tool as an interactive debugging application and
point of reference.

NOTE: should the API use byte arrays or hex strings?

The API should be well documented with all arguments explained. Likewise for the
commands help text.

Command cheatsheets and example sessions are strongly encouraged.

