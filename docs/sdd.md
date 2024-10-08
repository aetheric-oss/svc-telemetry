![Aetheric Banner](https://github.com/aetheric-oss/.github/raw/main/assets/doc-banner.png)

# Software Design Document (SDD) - `svc-telemetry`

## :telescope: Overview

This document details the software implementation of the Arrow telemetry service.

This service aggregates telemetry transmitted by networked assets (e.g.
aircraft, drones, mobile vertiports, etc.) and rebroadcasts to listeners.

It implements a cache to filter out duplicate telemetry reports, such as an
aircraft ADS-B message received by multiple networked towers within range.

### Metadata

| Attribute     | Description                                                       |
| ------------- |-------------------------------------------------------------------|
| Maintainer(s) | [Aetheric Realm Team](https://github.com/orgs/aetheric-oss/teams/dev-realm) |
| Stuckee       | A.M. Smith ([@amsmith-pro](https://github.com/amsmith-pro))         |
| Status        | Development                                                       |

## :books: Related Documents

Document | Description
--- | ---
[High-Level Concept of Operations (CONOPS)](https://github.com/aetheric-oss/se-services/blob/develop/docs/conops.md) | Overview of Aetheric microservices.
[High-Level Interface Control Document (ICD)](https://github.com/aetheric-oss/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Aetheric microservices.
[Requirements - `svc-telemetry`](https://nocodb.aetheric.nl/dashboard/#/nc/view/6ffa7547-b2ab-4d02-b5cb-ed2d3c60e2c7) | Requirements and user stories for this microservice.
[Concept of Operations - `svc-telemetry`](./conops.md) | Defines the motivation and duties of this microservice.
[Interface Control Document (ICD) - `svc-telemetry`](./icd.md) | Defines the inputs and outputs of this microservice.

## :dna: Module Attributes

Attribute | Applies | Explanation
--- | --- | ---
Safety Critical | Y | Live telemetry instrumental to safe operations, especially for autonomous vehicles.
Real-Time | Y | Telemetry broadcasts should be as close to realtime as possible, for safety concerns.

## :gear: Logic

### Initialization

At initialization this service creates two servers on separate threads: a GRPC server and a REST server.

The REST server expects the following environment variables to be set:
- `DOCKER_PORT_REST` (default: `8000`)

The GRPC server expects the following environment variables to be set:
- `DOCKER_PORT_GRPC` (default: `50051`)

### Control Loop

As a REST and GRPC server, this service awaits requests and executes handlers.

Some handlers **require** the following environment variables to be set:
- `STORAGE_HOST_GRPC`
- `STORAGE_PORT_GRPC`

This information allows this service to connect to other microservices to obtain information requested by the client.

:exclamation: These environment variables will *not* default to anything if not found. In this case, requests involving the handler will result in a `503 SERVICE UNAVAILABLE`.

For detailed sequence diagrams regarding request handlers, see [REST Handlers](#mailbox-rest-handlers).

## :mailbox: REST Handlers

### `adsb` Handler

The client will attempt to post a packet conforming to [ADS-B protocol](https://airmetar.main.jp/radio/ADS-B%20Decoding%20Guide.pdf).

**(adsb) Nominal**
```mermaid
sequenceDiagram
    autonumber
    participant client as Networked Node
    participant service as svc-telemetry
    participant redis as Redis Cache
    participant storage as svc-storage
    client-->>service: (REST) POST /telemetry/adsb
    Note over service: Create key from ADS-B:<br>ICAO address and calculated CRC32
    service->>redis: INCR key<br>PEXPIRE KEY 5000
    Note over redis: If key doesn't exist,<br>inserts with a value of 1.
    redis-->>service: N if N == (Value of this key in the cache)
    alt N == 1
        service-->>storage: Push raw packet and metadata fields
        storage-->>service: Success or Failure
    end
    service-->>client: (REST) Reply: N
```

**(adsb) Off-Nominal**: Invalid packet

Invalid request packets will return `400 BAD REQUEST`.

**(adsb) Off-Nominal**: Redis Cache Error

If there was an issue updating the Redis cache, the server will reply an opaque `500 INTERNAL_SERVER_ERROR`.

### `login` Handler

The client will attempt to obtain a JWT token.

```mermaid
sequenceDiagram
    autonumber
    participant client as vehicle
    participant service as svc-telemetry
    client-->>service: (REST) GET /telemetry/login<br>Aircraft Id String in Request Body
    alt invalid identifier string
        service-->>client: 400 BAD REQUEST
    end
    note over service: Create JWT claim with internal secret key
    service-->>client: Return encoded JWT key
```

:exclamation: This is not the final login scheme. In the future certificates will be used to ensure that the aircraft is who it reports to be.

### `network_remote_id` Handler

The client will attempt to post a packet conforming to remote ID protocol.

An encoded JWT 'Bearer' token (obtained through the `/telemetry/login/` interface) needs to be provided.

```mermaid
sequenceDiagram
    autonumber
    participant client as Networked Node
    participant service as svc-telemetry
    participant redis as Redis Cache
    participant gis as svc-gis
    client-->>service: (REST) POST /telemetry/netrid
    alt no auth token
        service-->>client: 401 UNAUTHORIZED
    end
    note over service: decode JWT key, verify
    alt invalid auth token
        service-->>client: 401 UNAUTHORIZED
    end
    note over service: process provided packet
    alt invalid packet size
        service-->>client: 400 BAD_REQUEST
    end
    Note over service: Create key from netrid packet
    service->>redis: INCR key<br>PEXPIRE KEY
    Note over redis: If key doesn't exist,<br>inserts with a value of 1.
    redis-->>service: N if N == (Value of this key in the cache)
    alt N == 1
        service-->>gis: Send processed position<br>or velocity
        service-->>rabbitmq: Publish telemetry
    end
    service-->>client: (REST) Reply: N
```
