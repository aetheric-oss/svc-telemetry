![Arrow Banner](https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png)

# Concept of Operations - `svc-telemetry`

## :telescope: Overview

This microservice exposes a public API to record telemetry. It processes and moves telemetry to various storage platforms.

This microservice additionally exposes interfaces to request recent aircraft
telemetry. It streams processed telemetry to other microservices in need of
up-to-date aircraft status (such as `svc-scheduler` and `svc-guidance`).

### Metadata

| Attribute     | Description                                                       |
| ------------- |-------------------------------------------------------------------|
| Maintainer(s) | [Services Team](https://github.com/orgs/Arrow-air/teams/services) |
| Stuckee       | A.M. Smith ([@ServiceDog](https://github.com/servicedog))         |
| Status        | Development                                                       |

## :books: Related Documents

Document | Description
--- | ---
[High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.
[High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md) | Interfaces and frameworks common to all Arrow microservices.
[Requirements - `svc-telemetry`](https://nocodb.arrowair.com/dashboard/#/nc/view/6ffa7547-b2ab-4d02-b5cb-ed2d3c60e2c7) | Requirements and user stories for this microservice.
[Interface Control Document (ICD) - `svc-telemetry`](./icd.md) | Defines the inputs and outputs of this microservice.
[Software Design Document (SDD) - `svc-telemetry`](./sdd.md) | Specifies the internal activity of this microservice.

## :raised_hands: Motivation

Live telemetry can be used to update itineraries, confirm departure and arrival, predict unsafe conditions, and inform a customer of flight status.

Other microservices (such as `svc-guidance` and `svc-scheduler`) may request real-time flight status and telemetry to determine or improve routing.

## Needs, Goals and Objectives of Envisioned System

`svc-telemetry` should expose a public API for:
- Networked assets (vertiports, aircraft) to post telemetry data.
- Authorized users to stream telemetry data.
- Authorized users to request specific telemetry data.

It should expose a private API for:
- Other microservices to request flight status (e.g. `svc-scheduler`).
- Other microservices to stream telemetry data (e.g. `svc-guidance`).

If telemetry is not being actively pushed to the network, `svc-telemetry` should subscribe to existing ADS-B streams from third-party providers, such as [Aerion](https://aireon.com/), [flightradar24](https://www.flightradar24.com/), or [FlightAware](https://flightaware.com).
- These services cost a subscription fee but provide wide coverage.
- These services receive their ADS-B telemetry from proprietary facilities and through public crowdsourcing
    - Individuals with software-defined radio USB dongles can forward capture ADS-B to the service in return for enterprise features on the service.
- They process the data, store it, and provide "real-time" streams to subscribers.

## External Interfaces

See the ICD for this microservice.

## Telemetry Types

The types of telemetry supported are:
- [ADS-B](https://www.faa.gov/air_traffic/technology/adsb)
    - Conventional aircraft and ground telemetry
    - ADS-B 1090 MHz broadcasts are required for all aircraft in USA and Europe.
    - [ADS-B Formation](https://www.mathworks.com/help/supportpkg/rtlsdrradio/ug/airplane-tracking-using-ads-b-signals.html)
    - `svc-telemetry` also accepts the UUID of the sender that captured the ADS-B message.
- [CCSDS](https://public.ccsds.org/Pubs/133x0b2e1.pdf)
    - Typically used in space applications.
    - Data segment containing [Arrow-defined telemetry formats](https://nocodb.arrowair.com/dashboard/#/nc/view/426aa4a3-1f74-43b0-b765-0b448be51242) for eVTOL.

## Technical Impacts

Asset managers have the choice to stream ADS-B directly to the `svc-telemetry` microservice, allowing the Arrow network to respond faster to real-time events than data that goes through a third-party ADS-B provider.

Real-time telemetry data may be considered by `svc-scheduler` when calculating the estimated trip length. In a learning system, sufficient telemetry data may come to dissuade routes with excessive turbulence or headwinds.

The collection, storage, and providing of telemetry could be a point of revenue for the network. Would require authentication enforcement for API requests to specific endpoints.

## Physical Environment

See the High-Level CONOPS.

## Support Environment

See the High-Level CONOPS.

## Environmental Impacts

**Energy Efficiency**

Telemetry can be used to refine routing and improve aircraft energy efficiency. For example, aircraft experiencing increased turbulence along a specific route will report sudden elevation changes in telemetry, which may inform future routes to avoid the region.

**Fire Alerts**

Real-time aircraft status can alert the network to aircraft crashes or other fire hazards that may pose a danger to the environment.

## Organizational Impacts

**Vertiport Managers**

Vertiport managers may track incoming aircraft through real-time telemetry and status streams.

**Sales**

Telemetry data may be marketable. Thorough business research may be conducted to pursue potential customers and correctly price data.

## Risks and Potential Issues

**General Failures**

A failure of this component could hinder the ability of `svc-scheduler` or `svc-guidance` to avoid unsafe flight conditions.

**Data Corruption**

Corrupt telemetry (such as bit flips) could appear to the system as if the asset has moved, had sudden acceleration, or has a new status. This could cause issues in `svc-scheduler` or `svc-guidance`. Checksums should be enforced on all received packets.

## Appendix A: Acronyms & Glossary

See [Arrow Glossary](https://www.arrowair.com/docs/documentation/glossary).
