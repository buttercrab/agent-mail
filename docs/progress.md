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
  - Imported the Rust server crate and smoke scripts from `/Users/jaeyong/skills/skills/agent-mail`.
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

### 2026-05-06 - Remote CI smoke fix

- Done:
  - Investigated failed remote CI run `25419056497`.
  - Identified failure in `make real-test` while starting temporary PostgreSQL on the GitHub Ubuntu runner.
  - Updated smoke scripts to use a temp-directory Unix socket with `postgres -k`.
  - Updated failure cleanup to print PostgreSQL and server logs.
  - Re-ran local `make ci` after the script change.
- Evidence:
  - Failed GitHub log showed `pg_ctl: could not start server` in `Real PostgreSQL HTTP/MCP smoke tests`.
  - Local `bash -n scripts/*.sh && make ci` passed after the change.
- Risk:
  - The remote CI fix still needs confirmation from a new GitHub Actions run.
- Next:
  - Push the smoke-script fix.
  - Wait for the new remote CI run and inspect any real failure logs.

### 2026-05-06 - Remote CI green

- Done:
  - Pushed smoke-script fix commit `4c4817f`.
  - Waited for the new GitHub Actions `CI` run on `main`.
- Evidence:
  - GitHub Actions run `25419133961` completed successfully.
  - The successful job included:
    - Install PostgreSQL binaries
    - Check formatting
    - Clippy
    - Rust tests
    - Real PostgreSQL HTTP/MCP smoke tests
- Risk:
  - Staging deploy is still not configured.
  - Docker build is still unverified because the local Docker daemon is unavailable.
  - Dependabot opened dependency PRs whose initial checks failed before the CI smoke-script fix reached their branches.
- Next:
  - Update Dependabot PR branches against current `main`.
  - Merge only PRs whose checks pass.
  - Configure real staging infrastructure/secrets.

### 2026-05-06 - Dependabot PRs handled

- Done:
  - Updated Dependabot PR branches against current `main`.
  - Merged PRs with successful real CI:
    - #1 `softprops/action-gh-release` 2 to 3
    - #2 `actions/checkout` 5 to 6
    - #4 `tower-http` 0.6.8 to 0.6.9
    - #5 `tokio` 1.52.1 to 1.52.2
  - Investigated PR #3 `rand` 0.9.4 to 0.10.1 after CI failure.
  - Updated `id.rs` on PR #3 for the `rand` 0.10 API (`SysRng` and `TryRng`).
  - Ran local `make ci` on PR #3 and waited for GitHub CI to pass before merging.
- Evidence:
  - PR #3 failure log showed unresolved `TryRngCore` and `OsRng` imports.
  - PR #3 GitHub CI run `25419345181` passed after the fix.
  - Final `main` GitHub CI run `25419393345` passed after all Dependabot merges.
- Risk:
  - Dependency updates are merged, but no tagged release has been cut.
  - Staging and production deploy workflows remain unproven.
- Next:
  - Configure staging infrastructure and GitHub environment secrets.
  - Run staging workflow manually and record evidence.

### 2026-05-06 - Staging provider access check

- Done:
  - Checked whether staging can be provisioned with currently available local credentials.
  - Added `docs/staging-setup.md` with the exact required infrastructure, GitHub secrets, DNS/TLS requirements, and verification gate.
- Evidence:
  - `aws sts get-caller-identity` failed with: `Your session has expired. Please reauthenticate using 'aws login'.`
  - `wrangler whoami` failed with: `Not logged in.`
  - Production SSH access still works, and `agent-mail-server.service` is active on the production host.
- Risk:
  - A true staging public edge cannot be created or verified without AWS and Cloudflare access, or equivalent staging host/DNS details from the operator.
  - Same-host staging may be technically possible, but it still needs DNS/TLS configuration for `staging.agent-mail.cc` to satisfy real deployed MCP/SSE validation.
- Next:
  - Reauthenticate AWS and Cloudflare, or provide staging host/DNS/token details.
  - Configure GitHub `staging` environment secrets.
  - Run the manual `Staging Deploy` workflow and record real evidence.

### 2026-05-06 - Repository protection

- Done:
  - Enabled branch protection for `main`.
  - Added repository description and topics.
- Evidence:
  - Branch protection now requires status check `Rust checks and real smoke tests`.
  - Required status checks are strict, so branches must be up to date.
  - Admin enforcement is enabled.
  - Linear history is required.
  - Force pushes and branch deletion are disabled.
  - Conversation resolution is required.
  - Repository topics are `agents`, `mcp`, `postgres`, and `rust`.
- Risk:
  - Branch protection does not replace staging validation; staging remains blocked by missing provider authentication or staging infrastructure details.
- Next:
  - Land this progress update through the protected branch flow.
  - Continue with real staging setup after AWS and Cloudflare authentication are available.

### 2026-05-06 - Staging isolation hardening

- Done:
  - Updated the staging workflow to require same-host staging isolation values before it can deploy.
  - Staging deploy now rejects the production service name, production install root, production source path, production private port, and production URL.
  - Added `AGENT_MAIL_ENVIRONMENT` to server config and `/health` so deployed smoke tests can prove they are hitting staging.
  - Updated deployed MCP smoke to optionally require an expected URL host and environment.
  - Updated deployed MCP smoke to check TCP reachability of the configured private service port instead of only checking whether `8787/health` returns `200`.
- Evidence:
  - `make ci` passed after the changes:
    - `cargo fmt --all -- --check`
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
    - `cargo test --workspace`, reporting zero Rust unit tests
    - `scripts/real_postgres_http_test.sh`
    - `scripts/real_postgres_mcp_test.sh`
  - `actionlint` passed.
  - `bash -n scripts/*.sh && git diff --check` passed.
- Risk:
  - These are repository/workflow hardening changes only; staging infrastructure still has to be provisioned and validated through the public edge.
- Next:
  - Land this hardening through a protected-branch PR.
  - Provision same-host staging with `/opt/agent-mail-staging`, `agent-mail-server-staging.service`, `127.0.0.1:8788`, `AGENT_MAIL_ENVIRONMENT=staging`, and separate PostgreSQL credentials.
  - Set the GitHub `staging` environment secrets/variables and run the manual `Staging Deploy` workflow.

### 2026-05-06 - First staging workflow run

- Done:
  - Created isolated staging PostgreSQL role/database:
    - database `agentmail_staging`
    - owner `agentmail_staging`
  - Installed staging host prerequisites:
    - `/etc/agent-mail/agent-mail-staging.env`
    - `/etc/systemd/system/agent-mail-server-staging.service`
    - `/etc/nginx/conf.d/agent-mail-staging.conf`
    - Cloudflare Origin certificate/key for `staging.agent-mail.cc`
  - Confirmed Cloudflare DNS for `staging.agent-mail.cc` points to the Lightsail IP and is proxied.
  - Configured GitHub `staging` environment secrets/variables.
  - Ran manual staging workflow `25420246932`.
- Evidence:
  - Cloudflare DNS API returned `staging.agent-mail.cc` content `100.22.38.210`, proxied `true`.
  - Host checks showed production `agent-mail-server.service` active and staging service enabled.
  - Main CI run `25420205581` passed after PR #7 merged.
  - Staging workflow `25420246932` failed during deploy with `cd: /opt/agent-mail-staging/src: Permission denied`.
- Risk:
  - Staging is still not validated; the failure happened before service restart and public MCP smoke.
- Next:
  - Fix staging deploy permissions so the SSH deploy user can build in the isolated staging source tree.
  - Re-run the manual staging workflow and record smoke evidence only if it passes.

### 2026-05-06 - Staging deployed and validated

- Done:
  - Fixed staging deploy permissions through PR #8.
  - Waited for main CI after PR #8 merge.
  - Re-ran the manual `Staging Deploy` workflow.
  - Confirmed staging and production services are both active on the host.
  - Confirmed staging and production listen only on loopback private ports.
- Evidence:
  - PR #8 CI run `25420308195` passed.
  - Main CI run `25420344470` passed after PR #8 merged.
  - Manual staging workflow run `25420377702` passed in GitHub Actions.
  - Staging smoke output:
    - project `public-mcp-20260506064152-2397`
    - receiver `bright-light-72d8c0f1`
    - mail id `mail-20260506-064157-b3d8262a8067d1ed`
  - `curl -fsS https://staging.agent-mail.cc/health` returned `{"environment":"staging","ok":true}`.
  - `curl -fsS https://agent-mail.cc/health` returned `{"ok":true}`; production is still serving the previous deployed binary.
  - Remote host check showed:
    - `agent-mail-server.service` active
    - `agent-mail-server-staging.service` active
    - listeners on `127.0.0.1:8787` and `127.0.0.1:8788`
  - Local TCP checks to `100.22.38.210:8787` and `100.22.38.210:8788` both failed to connect, confirming the raw service ports are not publicly reachable.
- Risk:
  - Production deploy workflow is still not proven from this repository.
  - Docker build remains unverified because the local Docker daemon was unavailable earlier.
- Next:
  - Land this progress update through the protected branch flow.
  - Configure production GitHub environment secrets/variables if not already present.
  - Cut a test release or manually deploy a selected ref to production, then run real production smoke and record evidence.

### 2026-05-06 - Production deploy validated

- Done:
  - Added production deploy smoke hardening through PR #10 so the production workflow requires:
    - `AGENT_MAIL_URL=https://agent-mail.cc`
    - expected host `agent-mail.cc`
    - `/health` environment `production`
  - Created the GitHub `production` environment.
  - Set production GitHub environment secrets:
    - `PROD_HOST`
    - `PROD_SSH_USER`
    - `PROD_SSH_KEY`
    - `PROD_AGENT_MAIL_TOKEN`
    - `PROD_PUBLIC_IP`
  - Ran the manual `Production Deploy` workflow from `main`.
  - Confirmed production and staging stayed active after production deploy.
- Evidence:
  - PR #10 CI run `25420970576` passed.
  - Main CI run `25421012984` passed after PR #10 merged.
  - Manual production workflow run `25421069231` passed in GitHub Actions.
  - Production workflow checked out `main` at commit `80d55cb5d96a6e70fe797acb52e4eb1a67a39c44`.
  - Production deploy built `agent-mail-server` in release mode and restarted `agent-mail-server.service`.
  - Production smoke output:
    - project `public-mcp-20260506070129-2354`
    - receiver `steady-river-b5e51127`
    - mail id `mail-20260506-070144-d3aacb0bd2d8f1f7`
  - `curl -fsS https://agent-mail.cc/health` returned `{"environment":"production","ok":true}`.
  - `curl -fsS https://staging.agent-mail.cc/health` returned `{"environment":"staging","ok":true}`.
  - Remote host check showed:
    - `agent-mail-server.service` active
    - `agent-mail-server-staging.service` active
    - listeners on `127.0.0.1:8787` and `127.0.0.1:8788`
  - Local TCP checks to `100.22.38.210:8787` and `100.22.38.210:8788` both failed to connect.
- Risk:
  - Docker build is still unverified because the local Docker daemon was unavailable earlier.
  - No tagged release has been cut yet.
- Next:
  - Land this progress update through the protected branch flow.
  - Decide whether to require Docker build in CI or leave Docker as best-effort local packaging.
  - Cut the first tagged release if the current deployed commit is accepted as the initial release candidate.

### 2026-05-06 - Release validated

- Done:
  - Prepared `CHANGELOG.md` for `v0.1.0` through PR #12.
  - Tagged `v0.1.0` after main CI passed.
  - Verified the GitHub Release workflow.
- Evidence:
  - PR #12 CI run `25421383357` passed.
  - Main CI run `25421423387` passed after PR #12 merged.
  - Release workflow run `25421465058` passed.
  - GitHub release `v0.1.0` exists at `https://github.com/buttercrab/agent-mail/releases/tag/v0.1.0`.
  - Release assets:
    - `agent-mail-server-linux-x86_64`
    - `SHA256SUMS`
- Risk:
  - Docker build is still unverified because the local Docker daemon was unavailable earlier.
- Next:
  - Add Docker build validation to CI so packaging is tested on GitHub-hosted runners.

### 2026-05-06 - Docker build validated in CI

- Done:
  - Added `docker build --pull --tag agent-mail-server:ci .` to CI through PR #13.
  - Added `AGENT_MAIL_ENVIRONMENT=development` to `docker-compose.yml`.
  - Verified CI with Docker packaging enabled.
- Evidence:
  - PR #13 CI run `25421592122` passed and included the Docker build step.
  - Main CI run `25421705258` passed after PR #13 merged and included the Docker build step.
- Risk:
  - Local Docker remains unavailable on this workstation, but Docker packaging is now verified on GitHub-hosted runners.
- Next:
  - Keep Docker build in the required CI check.

### 2026-05-06 - Nano/RDS migration started

- Done:
  - Set the active goal to migrate toward the cheap Lightsail Nano + private RDS PostgreSQL architecture.
  - Chose the deploy strategy:
    - PRs run CI only with real temporary PostgreSQL smoke tests.
    - pushes to `main` deploy automatically to staging.
    - production deploy remains manual from a selected ref or tag.
  - Started workflow changes to make Nano viable:
    - CI, release, staging deploy, and production deploy use Cargo cache plus `sccache`.
    - Docker builds use GitHub Actions cache via BuildKit.
    - Dockerfile uses `cargo-chef` to cache dependency compilation layers separately from application source changes.
    - staging and production deploys build release binaries on GitHub-hosted runners.
    - staging and production hosts receive only the compiled `agent-mail-server` binary instead of building Rust on the instance.
- Evidence:
  - `aws sts get-caller-identity` failed with: `Your session has expired. Please reauthenticate using 'aws login'.`
  - Current production host architecture is `x86_64`, matching GitHub's Linux runner binary output.
- Risk:
  - RDS creation, VPC peering, RDS security groups, DB dump/restore, and Lightsail Nano resize cannot proceed until AWS auth is refreshed.
- Next:
  - Land and verify the workflow/cache/deploy changes first.
  - After AWS auth is refreshed, provision RDS PostgreSQL `db.t4g.micro`, create separate prod/staging DBs and users, migrate data, validate staging, then promote production.

### 2026-05-06 - Cached runner deploys validated

- Done:
  - Landed PR #15 with cached GitHub-runner builds and binary-only host deploys.
  - Added `cargo-chef` to the Dockerfile so Docker dependency compilation can be cached separately from application source.
  - Enabled automatic staging deploy on pushes to `main`.
- Evidence:
  - PR #15 CI run `25423512183` passed after adding `cargo-chef`; the run included:
    - `sccache`
    - Cargo cache
    - real PostgreSQL HTTP/MCP smoke
    - Docker BuildKit build with `cargo-chef`
  - Main CI run `25423761896` passed after PR #15 merged.
  - Main staging deploy run `25423761898` passed after PR #15 merged.
  - Staging deploy used GitHub-runner release build plus binary upload, then passed deployed MCP smoke:
    - project `public-mcp-20260506080946-8928`
    - receiver `silver-ridge-8ebf4ff4`
    - mail id `mail-20260506-080950-2bdbe49b1f5a5d41`
  - Staging `sccache` stats showed a cold cache on first run: `0 hits`, `206 misses`, `0 errors`; the cache was written for later runs.
- Risk:
  - GitHub emitted a Node.js 20 deprecation warning for `mozilla-actions/sccache-action@v0.0.9`, `docker/setup-buildx-action@v3`, and `docker/build-push-action@v6`. The warning did not fail CI, but it should be monitored or addressed before GitHub removes Node 20 support.
  - AWS auth is still expired, so RDS/Nano provisioning remains blocked.
- Next:
  - Refresh AWS auth.
  - Provision private RDS PostgreSQL and migrate staging first.

### 2026-05-06 - Private RDS and Nano migration completed

- Done:
  - Provisioned private AWS RDS PostgreSQL `agent-mail-rds-20260506` in `us-west-2`.
  - Enabled Lightsail VPC peering to the default VPC.
  - Created RDS security group `sg-0782b8f8e7f5509b6` for `tcp/5432`.
  - Created separate RDS databases and users for production and staging.
  - Migrated staging from the old Lightsail managed PostgreSQL database to RDS first, then verified staging through the real GitHub staging workflow.
  - Migrated production to RDS during a short service freeze, with a final logical dump and exact row-count reconciliation.
  - Created rollback anchors:
    - Lightsail managed DB snapshot `agent-mail-prod-pre-rds-20260506084541`.
    - RDS snapshot `agent-mail-rds-post-prod-cutover-20260506084828`.
  - Created fresh Lightsail Nano instance `agent-mail-20260506-nano` instead of attempting an invalid snapshot downsize.
  - Reassigned static IPv4 `100.22.38.210` to the Nano after direct-origin health checks passed.
  - Left the old small instance `agent-mail-20260504052845-web` running with both app services stopped for host-level rollback.
- Evidence:
  - RDS:
    - instance `agent-mail-rds-20260506`
    - engine PostgreSQL `18.3`
    - `PubliclyAccessible=false`
    - encrypted storage enabled
    - deletion protection enabled
    - endpoint `agent-mail-rds-20260506.ch8s8kymek34.us-west-2.rds.amazonaws.com`
  - RDS security group:
    - `172.26.8.117/32` kept temporarily for old-host rollback
    - `172.26.1.42/32` added for active Nano host
  - Staging RDS migration:
    - source frozen counts before restore: `messages=3`, `participants=6`, `projects=3`, `receipts=3`
    - RDS restored counts: `messages=3`, `participants=6`, `projects=3`, `receipts=3`
    - GitHub staging deploy run `25425172057` passed after switching staging to RDS.
    - staging smoke IDs: project `public-mcp-20260506084156-2606`, receiver `swift-light-3f2cae32`, mail id `mail-20260506-084200-da51aa193df75f0b`
  - Production RDS migration:
    - service freeze started `2026-05-06T08:46:29Z`
    - `pg_dump`, `pg_restore`, and `psql` were PostgreSQL `18.3`
    - source frozen counts: `messages=38`, `participants=42`, `projects=20`, `receipts=36`
    - dump file `/tmp/agent-mail-prod-rds-20260506084629.dump`
    - dump size `13267` bytes
    - dump SHA-256 `b09ccfbc3d3bec4dd26ec5ca06255124e008319b0f77f58dacb4ea57f62af910`
    - restored RDS counts before smoke matched exactly.
    - GitHub production deploy run `25425410316` passed after production was switched to RDS.
    - production workflow smoke IDs: project `public-mcp-20260506084737-6412`, receiver `crisp-light-093939de`, mail id `mail-20260506-084740-05385c0197470d09`
  - Nano cutover:
    - active app host `agent-mail-20260506-nano`
    - bundle `nano_3_0`
    - public/static IPv4 `100.22.38.210`
    - private IP `172.26.1.42`
    - direct-origin health before static IP move passed for both `agent-mail.cc` and `staging.agent-mail.cc`
    - public production health after cutover returned `{"environment":"production","ok":true}`
    - public staging health after cutover returned `{"environment":"staging","ok":true}`
    - production MCP smoke after Nano cutover passed: project `public-mcp-20260506015313-27815`, receiver `warm-signal-d356945c`, mail id `mail-20260506-085321-4f4c6ed4b9583b94`
    - staging MCP smoke after Nano cutover passed: project `public-mcp-20260506015335-27985`, receiver `steady-cloud-3afe8054`, mail id `mail-20260506-085341-53dadafd6c92b3b3`
    - final RDS-backed production counts after all post-cutover smokes: `messages=40`, `participants=46`, `projects=22`, `receipts=38`
    - final RDS-backed staging counts after Nano smoke: `messages=5`, `participants=10`, `projects=5`, `receipts=5`
    - raw public checks to `100.22.38.210:8787`, `100.22.38.210:8788`, and `100.22.38.210:5432` timed out.
    - old small instance dynamic IP `52.40.39.158` has both `agent-mail-server.service` and `agent-mail-server-staging.service` inactive.
  - CI/deploy cache evidence:
    - Production deploy run `25425410316` showed `sccache` `206 hits`, `0 misses`, `0 errors`.
- Risk:
  - This is a cheap single-instance app host plus single-AZ RDS shape, not high availability.
  - At cutover time, the old small instance and old Lightsail managed DB were retained for rollback and still cost money until retired.
  - At cutover time, the RDS security group still included the old small instance private IP for rollback.
  - GitHub still emits a Node.js 20 deprecation warning for `mozilla-actions/sccache-action@v0.0.9`.
- Next:
  - Observe production and staging on Nano.
  - Retire the old small instance, old Lightsail managed database, and old RDS security group ingress after the rollback window.
  - Land this progress update through the protected branch flow.

### 2026-05-06 - Legacy Lightsail resources removed

- Done:
  - Deleted legacy Lightsail app instance `agent-mail-20260504052845-web`.
  - Deleted legacy Lightsail managed PostgreSQL database `agent-mail-20260504052845-db` with `--skip-final-snapshot`.
  - Deleted legacy Lightsail managed DB snapshot `agent-mail-prod-pre-rds-20260506084541`.
  - Removed old app host private IP `172.26.8.117/32` from RDS security group `sg-0782b8f8e7f5509b6`.
- Evidence:
  - `GET /health` still returned `{"environment":"production","ok":true}` for `https://agent-mail.cc`.
  - `GET /health` still returned `{"environment":"staging","ok":true}` for `https://staging.agent-mail.cc`.
  - `aws lightsail get-instance --instance-name agent-mail-20260504052845-web` returned `NotFoundException`.
  - `aws lightsail get-relational-database --relational-database-name agent-mail-20260504052845-db` returned `NotFoundException`.
  - Legacy Lightsail DB snapshot query returned `[]`.
  - RDS security group ingress now allows only `172.26.1.42/32` on `tcp/5432`.
- Risk:
  - Host-level rollback to the old Lightsail small instance is no longer available.
  - The retained rollback anchor is the current RDS snapshot `agent-mail-rds-post-prod-cutover-20260506084828` plus RDS automated backups.
- Next:
  - Keep observing production and staging on the Nano/RDS shape.
  - Remove obsolete local notes or credentials only after confirming they are not still used by deploy workflows or recovery docs.

### 2026-05-06 - Root crate refactor started

- Done:
  - Flattened the Rust service from `rust/agent-mail-server/src` into root-level `src`.
  - Converted the root `Cargo.toml` from a virtual workspace manifest into the `agent-mail-server` package manifest.
  - Updated local, CI, Docker, staging, production, and release build commands to target the root package directly.
  - Added explicit config validation for required runtime values and non-empty allowed origins.
  - Avoided bearer-token authorization string allocation.
  - Preserved database errors during generated identity allocation instead of treating every error as an available identity.
  - Canonicalized whitespace-trimmed identifiers before store lookups and writes.
  - Replaced response serialization `unwrap` calls in MCP handling with typed internal errors.
  - Added unit tests for MCP URI percent-encoding and resource URI parsing.
- Evidence:
  - `cargo metadata --no-deps --format-version 1` reports package `agent-mail-server` at root `Cargo.toml` with target `src/main.rs`.
  - `cargo test` passed with 4 Rust unit tests.
  - `cargo clippy --all-targets --all-features -- -D warnings` passed.
  - `make real-test` passed:
    - `scripts/real_postgres_http_test.sh`
    - `scripts/real_postgres_mcp_test.sh`
  - `bash -n scripts/*.sh && git diff --check && actionlint` passed.
- Risk:
  - Local Docker build could not run because the Docker daemon socket was unavailable at `/Users/jaeyong/.docker/run/docker.sock`.
  - Docker build and deployed staging verification still need to run remotely before merging.
- Next:
  - Run the Docker build through CI.
  - Merge through the protected PR flow only after remote checks pass.
