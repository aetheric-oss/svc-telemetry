![Arrow Banner](https://github.com/Arrow-air/.github/raw/main/profile/assets/arrow_v2_twitter-banner_neu.png)

# `svc-telemetry`

![GitHub stable release (latest by date)](https://img.shields.io/github/v/release/Arrow-air/svc-telemetry?sort=semver&color=green)
![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/Arrow-air/svc-telemetry?include_prereleases)
![Sanity Checks](https://github.com/arrow-air/svc-telemetry/actions/workflows/sanity_checks.yml/badge.svg?branch=develop)
![Rust Checks](https://github.com/arrow-air/svc-telemetry/actions/workflows/rust_ci.yml/badge.svg?branch=develop)
![Python PEP8](https://github.com/arrow-air/svc-telemetry/actions/workflows/python_ci.yml/badge.svg?branch=develop)
![Arrow DAO
Discord](https://img.shields.io/discord/853833144037277726?style=plastic)

## :telescope: Overview

This module is responsible for aggregating ADS-B messages from numerous external senders and rebroadcasting a stream (without duplicates) to authenticated listeners, including some within the Arrow network.
