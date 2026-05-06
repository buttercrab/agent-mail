# Changelog

All notable changes to Agent Mail will be documented here.

This project follows semantic versioning after the first tagged release.

## v0.1.0 - 2026-05-06

- Imported Agent Mail service into dedicated repository.
- Added durable setup plan, progress tracker, and decision records.
- Added strict CI with formatting, clippy, Rust tests, and real PostgreSQL HTTP/MCP smoke tests.
- Added staging and production deploy workflows with real public MCP/SSE smoke validation.
- Added separate same-host staging deployment using an isolated PostgreSQL database, systemd service, bind port, install root, token, DNS name, and Cloudflare Origin certificate.
- Added `/health` environment reporting so deploy smoke tests can reject the wrong environment.
- Added OSS project baseline documentation, security policy, contribution guide, Docker packaging files, Dependabot, issue templates, PR template, and CODEOWNERS.
