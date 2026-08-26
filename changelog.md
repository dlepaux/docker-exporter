## [1.5.2](https://github.com/dlepaux/docker-exporter/compare/v1.5.1...v1.5.2) (2026-08-26)


### Bug Fixes

* **deps:** update cargo non-major ([#17](https://github.com/dlepaux/docker-exporter/issues/17)) ([ca18bd0](https://github.com/dlepaux/docker-exporter/commit/ca18bd05eed988285c999be3d5f8e4eeacb79b12))

## [1.5.1](https://github.com/dlepaux/docker-exporter/compare/v1.5.0...v1.5.1) (2026-08-17)


### Bug Fixes

* **renovate:** move prPriority out of vulnerabilityAlerts ([451e03e](https://github.com/dlepaux/docker-exporter/commit/451e03e98f560ad51057507ddcfc3127ab75f179))

# [1.5.0](https://github.com/dlepaux/docker-exporter/compare/v1.4.2...v1.5.0) (2026-07-31)


### Features

* **metrics:** emit container_exit_code for terminal containers ([ed8c3b2](https://github.com/dlepaux/docker-exporter/commit/ed8c3b25335640691832dc8581faad05e88f19b1))

## [1.4.2](https://github.com/dlepaux/docker-exporter/compare/v1.4.1...v1.4.2) (2026-07-09)


### Bug Fixes

* **collector:** bound per-container fan-out to stop fd exhaustion ([7c66fc2](https://github.com/dlepaux/docker-exporter/commit/7c66fc231c3a93e7f5cac4dada6e146de0093c80))

## [1.4.1](https://github.com/dlepaux/docker-exporter/compare/v1.4.0...v1.4.1) (2026-07-07)


### Bug Fixes

* **deps:** patch-release the anyhow 1.0.103 security bump (RUSTSEC-2026-0190) ([23f2cb4](https://github.com/dlepaux/docker-exporter/commit/23f2cb476b23e5dfea555c7687edc45615c68bf1)), closes [#2](https://github.com/dlepaux/docker-exporter/issues/2)

# [1.4.0](https://github.com/dlepaux/docker-exporter/compare/v1.3.0...v1.4.0) (2026-06-18)


### Features

* **metrics:** emit restart_policy label on container_state ([88c1bef](https://github.com/dlepaux/docker-exporter/commit/88c1bef61303a0f082b7fda6d0bd82f57635695f))

# [1.3.0](https://github.com/dlepaux/docker-exporter/compare/v1.2.0...v1.3.0) (2026-06-03)


### Features

* ship musl static binary on distroless + native --health check ([10ffcd2](https://github.com/dlepaux/docker-exporter/commit/10ffcd24a0a7f0f558e826919991e05dae5172d8))

# [1.2.0](https://github.com/dlepaux/docker-exporter/compare/v1.1.0...v1.2.0) (2026-06-03)


### Features

* support glob patterns in EXCLUDE_CONTAINERS ([c43801c](https://github.com/dlepaux/docker-exporter/commit/c43801c25875c37a1dc3aa3bf78c96af3a6bea47))

# [1.1.0](https://github.com/dlepaux/docker-exporter/compare/v1.0.1...v1.1.0) (2026-06-02)


### Bug Fixes

* **test:** drop per-container assertion that fails on zero-container CI Docker ([1307be4](https://github.com/dlepaux/docker-exporter/commit/1307be4dae29945bf94ac63958a3397d40e13382))


### Features

* **collector:** collect container health via inspect + inspect-failure counter ([25e6f6f](https://github.com/dlepaux/docker-exporter/commit/25e6f6f3a6d3659adddae5b45864d2c325588a3a))
* **metrics:** emit container_health_status gauge + inspect-failures counter ([698aa9c](https://github.com/dlepaux/docker-exporter/commit/698aa9cde29e4a3c8f9ade501334b5221e145843))

## [1.0.1](https://github.com/dlepaux/docker-exporter/compare/v1.0.0...v1.0.1) (2026-05-31)


### Bug Fixes

* **docker:** patch base OS packages on build ([6d30b02](https://github.com/dlepaux/docker-exporter/commit/6d30b02d193b0ee69fec3185984e81df4bf8af56))

# 1.0.0 (2026-04-25)


### Features

* initial public release ([f1b455a](https://github.com/dlepaux/docker-exporter/commit/f1b455a37b382fd2b79dbc54b0141c06d72593eb))

# Changelog

All notable changes to this project will be documented in this file.

This file is automatically updated by [release-please](https://github.com/googleapis/release-please) on each release.
