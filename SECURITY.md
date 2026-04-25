# Security Policy

## Supported versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x     | :white_check_mark: |
| < 1.0   | :x:                |

`docker-exporter` follows semantic versioning. Security patches land on the
latest `1.x` release. Older majors are unsupported once a new major ships.

## Reporting a vulnerability

**Please do not open a public GitHub issue for security reports.**

Two private channels are accepted, in order of preference:

1. **GitHub Private Vulnerability Reporting** — preferred.
   Open the [Security tab](https://github.com/dlepaux/docker-exporter/security)
   on this repository and use *Report a vulnerability*. This gives both sides
   a structured workflow, advisory drafting, and CVE issuance if applicable.
2. **Email** — `d.lepaux@gmail.com` with subject prefix
   `[docker-exporter security]`. PGP is not currently offered; if your report
   contains exploit details, encrypt at the transport layer (e.g. ProtonMail
   to ProtonMail) or send only enough detail to coordinate a private channel.

Please include:

- Affected version(s) (output of `docker-exporter --version` or image tag)
- Reproduction steps or proof-of-concept
- Expected vs actual behaviour
- Your assessment of impact (information disclosure, RCE, DoS, etc.)

## Threat model

- **Listen address is the trust boundary.** The exporter binds
  `0.0.0.0:9713` by default. Anyone who can reach that port can read
  container `name`/`id`/`image` labels and resource metrics. Operators
  are expected to firewall the port, bind to `127.0.0.1`, or expose only
  on a private network — the exporter itself does no authentication.
- **Docker socket is read-only by convention.** The recommended mount
  (`/var/run/docker.sock:/var/run/docker.sock:ro`) prevents writes at the
  kernel level. The exporter only calls `list_containers`, `stats`, and
  `ping` — there are no write paths in the code regardless.
- **Container metadata is trusted, with a narrow surface.** Docker
  validates container names against `[a-zA-Z0-9][a-zA-Z0-9_.-]+`, so
  newlines, quotes, and path traversal cannot reach `tracing` or
  Prometheus output via `name`. The labels emitted today are `name`,
  `id`, `image`, `interface`, and `operation` — all bounded character
  sets. Container *labels* (arbitrary UTF-8) are **not** emitted; if
  that surface is widened in the future, sanitization must be added at
  the encoder boundary before this assumption holds.

## Response SLA

This is a solo-maintained project. Best-effort timelines:

- **Acknowledgment**: within **72 hours** of report
- **Initial assessment** (severity, scope): within **7 days**
- **Patch released** for confirmed issues: target **30 days** for high/critical,
  **90 days** for low/medium. If a fix is not feasible within the window, the
  reporter is informed and a revised timeline is agreed.

## Disclosure

Coordinated disclosure with a **default 90-day** embargo from acknowledgment.
The window can be shortened (active exploitation) or extended (complex fix)
by mutual agreement. Once a patched release is published:

- A GitHub Security Advisory is published with credit to the reporter
  (unless anonymity is requested)
- A CVE is requested via GitHub for issues that warrant one
- The fix is referenced in `changelog.md` under the affected release

## Out of scope

Reports for the following are appreciated but not treated as security issues:

- Vulnerabilities in dependencies that have **no exploitable path** through
  this exporter (track via `cargo audit` and Renovate instead)
- Issues that require root access or `--privileged` containers — the threat
  model already assumes the operator controls the host
- Self-DoS scenarios (e.g. crafting a metric scrape that exhausts memory on
  the scraping Prometheus, not the exporter itself)

## Hardening this repo applies

- **Renovate `vulnerabilityAlerts`** — security advisories trigger immediate
  PRs and auto-merge on green CI, regardless of the routine schedule.
- **`cargo audit`** runs on every CI build; the build fails on unaddressed
  advisories. Ignored advisories are documented at the top of
  `audit.toml` (when present) or in CI invocation.
- **GitHub Actions pinned to commit SHA** with version comment for
  human-readable diffs. Renovate keeps SHAs current.
- **Container runs as non-root** (UID 1001) with read-only Docker socket.

## Acknowledgments

Reporters who follow this policy and act in good faith will be credited in
the published advisory and `changelog.md` unless they request otherwise.
