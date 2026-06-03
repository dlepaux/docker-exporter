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
