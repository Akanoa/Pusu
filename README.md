# PUṢŪ(PubSub as Service)

<img height="110" src="assets/doc/images/logo.png" width="90"/>

Puṣū is a distributed PubSub FoundationDB layer mimicking
the [Redis PubSub](https://redis.io/docs/latest/develop/interact/pubsub/)
but adding a tenant isolation leveraged by the [Biscuit](https://www.biscuitsec.org/) authentication/authorisation
token.

## Project overview

### Components

The Plushy project is divided in multiple crates, each one with their own purpose:

- [puṣū-server-lib](pusu-server-lib) : Handle the communication with the FoundationDB cluster
- [puṣū-server](pusu-server) : Listen to puṣū-client connections
- [puṣū-client-lib](pusu-client-lib) : Programmatic client
- [puṣū-client](pusu-client) : CLI client
- [puṣū-protocol](pusu-protocol) : Describes the protocol between client and server

### Relations between components

Each `puṣū-server` of a same cluster shared the same FoundationDB as storage endpoint.

`puṣū-server` are fully stateless components, they only hold a transient client session which can be relaunched
in an another `puṣū-server` instance. This architecture allows a horizontal-scaling preventing any trafic burst.

`puṣū-server` instances are meant to be used behind a load-balancer.

`puṣū-client` on its own, can be either some embedded library in another program, or the puṣū-client CLI.

`puṣū-client` connects to any `puṣū-server` and starts the communication using
the [puṣū-protocol](pusu-protocol).

![](assets/doc/images/overview.png)