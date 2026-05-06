# Agent Mail Repository Progress

This file is the durable handoff record. Update it after every meaningful milestone with real evidence.

## Current Goal

Set up this repository as a strict, production-grade OSS Rust/MCP service with real CI, staging deployment validation, production release flow, documentation, and durable progress tracking. Do not claim fake tests or assume deploy verification.

## Log

### 2026-05-06 - Repository inspection

- Done:
  - Confirmed repository path: `/Users/jaeyong/Development/Github/agent-mail`.
  - Confirmed repository is empty except for Git metadata.
  - Confirmed remote: `git@github.com:buttercrab/agent-mail.git`.
  - Confirmed source repository for import: `/Users/jaeyong/skills/skills/agent-mail`.
- Evidence:
  - `git status -sb` showed `## No commits yet on main...origin/main [gone]`.
  - `git remote -v` showed `origin git@github.com:buttercrab/agent-mail.git`.
  - Source files were listed from `/Users/jaeyong/skills/skills/agent-mail`.
- Risk:
  - The new repository has no commits yet.
  - Existing tests are smoke/integration shell tests; Rust unit test count is currently zero in the source service.
- Next:
  - Import the existing service into this repository without changing behavior.
  - Run the imported service checks from the new path and record actual output.

### 2026-05-06 - Plan/progress docs added

- Done:
  - Added `docs/plan.md`.
  - Added `docs/progress.md`.
  - Added `docs/decisions/` in the planned repository structure.
- Evidence:
  - Files added by patch in the new repository.
- Risk:
  - Docs are not validation; code import and real checks still remain.
- Next:
  - Add initial ADRs and import service files.

### 2026-05-06 - Initial service import and OSS baseline

- Done:
  - Imported Cargo workspace, Rust server crate, and smoke scripts from `/Users/jaeyong/skills/skills/agent-mail`.
  - Added `README.md`, `LICENSE`, `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `.env.example`, `.gitignore`, `rust-toolchain.toml`, `rustfmt.toml`, and `clippy.toml`.
  - Added GitHub issue templates, PR template, CODEOWNERS, and Dependabot config.
  - Added Dockerfile and Docker Compose for local service/PostgreSQL startup.
  - Added docs for MCP, testing, and deployment.
  - Added GitHub Actions workflows for CI, staging deploy, release, and manual production deploy.
  - Split deployed smoke testing into:
    - `scripts/deployed_mcp_smoke.sh` for any real deployed URL
    - `scripts/public_mcp_smoke.sh` for production `https://agent-mail.cc`
- Evidence:
  - Files exist in the new repository working tree.
  - `cargo fmt --all -- --check` passed.
  - `make test` passed and reported zero Rust unit tests.
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` failed on imported collapsible-if warnings before fixes.
- Risk:
  - GitHub Actions workflows have not run on GitHub yet.
  - Staging secrets and staging infrastructure have not been verified.
  - Docker build has not been run yet.
  - Production deploy workflow is untested in this repository.
- Next:
  - Re-run clippy after fixes.
  - Run `make real-test` from the new repository.
  - Run syntax checks for shell scripts.

### 2026-05-06 - Local validation evidence

- Done:
  - Fixed clippy `collapsible_if` failures in imported MCP code.
  - Added configurable `AGENT_MAIL_ALLOWED_ORIGINS` so staging can validate with its own public origin.
  - Ran the strict local CI target from this repository.
  - Ran GitHub Actions workflow lint.
  - Ran production deployed MCP smoke through the production wrapper.
- Evidence:
  - `make ci` passed:
    - `cargo fmt --all -- --check`
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
    - `cargo test --workspace`, reporting zero Rust unit tests
    - `scripts/real_postgres_http_test.sh`
    - `scripts/real_postgres_mcp_test.sh`
  - `actionlint` passed.
  - `git diff --check` passed.
  - `make public-mcp-smoke` passed using production environment values from the existing local env file:
    - project `public-mcp-20260505225728-12849`
    - receiver `warm-field-9afcf3fa`
    - mail id `mail-20260506-055734-f0a87600978b61f5`
- Risk:
  - Docker build did not run because the local Docker daemon was unavailable: `failed to connect to the docker API`.
  - GitHub Actions have not run remotely yet.
  - The first remote staging run failed because staging secrets/variables are not created. This is expected and proves staging is not configured.
  - Production deploy workflow is defined but has not deployed this new repository.
- Next:
  - Commit and push the initial repository setup.
  - Configure GitHub environments/secrets for staging and production.
  - Run remote GitHub CI and staging deploy before treating the repository setup as complete.

### 2026-05-06 - Remote initial push

- Done:
  - Created initial commit `11a8e12` and pushed `main` to `git@github.com:buttercrab/agent-mail.git`.
  - Checked GitHub workflow state after push.
- Evidence:
  - `git push -u origin main` succeeded.
  - GitHub repo resolved as `buttercrab/agent-mail`.
  - Remote staging run `25419056500` failed at the explicit required-secrets gate because all staging values were empty.
- Risk:
  - Staging cannot be considered set up until real infrastructure and GitHub environment secrets/variables exist.
- Next:
  - Make staging workflow manual-only until staging is actually configured.
  - Push that correction and check remote CI status.
