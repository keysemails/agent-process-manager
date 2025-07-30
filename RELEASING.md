# Releasing Agent Process Manager

This document describes the release process for Agent Process Manager (APM).

## Release Process

### 1. Update Version

Update the version in `Cargo.toml`:
```toml
[package]
version = "0.3.1"  # Update this
```

### 2. Update CHANGELOG

Update `CHANGELOG.md` with the new version and its changes. Follow the format:
```markdown
## [0.3.1] - 2025-07-19

### Added
- New features...

### Changed
- Changes to existing functionality...

### Fixed
- Bug fixes...
```

### 3. Commit Changes

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "Prepare release v0.3.1"
git push origin main
```

### 4. Create and Push Tag

```bash
git tag -a v0.3.1 -m "Release v0.3.1 - Brief description"
git push origin v0.3.1
```

This will trigger the GitHub Actions workflow that:
- Builds binaries for Linux x64, macOS x64, and macOS ARM64
- Creates a GitHub release with the binaries attached
- Builds and pushes a Docker image to `ghcr.io/sunnya97/apm:TAG`
- Generates installation instructions

### 5. Verify Release

1. Check the [Actions tab](https://github.com/sunnya97/agent-process-manager/actions) to ensure the workflow succeeded
2. Visit the [Releases page](https://github.com/sunnya97/agent-process-manager/releases) to verify the release was created
3. Verify the Docker image was published:
   ```bash
   docker pull ghcr.io/sunnya97/apm:v0.3.1
   docker run --rm ghcr.io/sunnya97/apm:v0.3.1 --version
   ```
4. Make the Docker package public (one-time setup):
   - Go to https://github.com/sunnya97/agent-process-manager/pkgs/container/apm
   - Click "Package settings" 
   - Change visibility to "Public"
   - This allows other projects to pull the image without authentication

## Version Numbering

We follow [Semantic Versioning](https://semver.org/):
- MAJOR version for incompatible API changes
- MINOR version for backwards-compatible functionality additions
- PATCH version for backwards-compatible bug fixes

## Pre-releases

For alpha/beta releases, use tags like:
- `v0.4.0-alpha.1`
- `v0.4.0-beta.1`
- `v0.4.0-rc.1`

## Using APM in Docker Containers

Since APM is in a private repository, the recommended way to use it in other Docker containers is via the public Docker image:

```dockerfile
# In your Dockerfile
COPY --from=ghcr.io/sunnya97/apm:v0.3.1 /usr/local/bin/apm /usr/local/bin/apm
```

Or use `latest` tag for the most recent version:
```dockerfile
COPY --from=ghcr.io/sunnya97/apm:latest /usr/local/bin/apm /usr/local/bin/apm
```

## Dependency on MCP Crate

APM depends on the MCP crate from git, which prevents publishing to crates.io:
```toml
[dependencies.rmcp]
git = "https://github.com/modelcontextprotocol/rust-sdk"
```

Until the MCP crate is published to crates.io, APM must be distributed as binary releases through GitHub.

## Updating Vibe Integration

When releasing a new version of APM that Vibe should use:

1. Update the Dockerfile in Vibe's machine-image:
   ```bash
   cd vibe/backend/machine-image
   ./update-apm-version.sh v0.3.1
   ```

2. Build and deploy the new Docker image:
   ```bash
   ./deploy-simple.sh
   ```

## Manual Release (Alternative)

If automatic releases fail, you can create a release manually:

1. Build binaries locally:
   ```bash
   # Linux (requires Docker)
   docker run --rm -v "$PWD:/app" -w /app rust:latest sh -c \
     "cargo build --release --target x86_64-unknown-linux-gnu"
   
   # macOS x64
   cargo build --release --target x86_64-apple-darwin
   
   # macOS ARM64
   cargo build --release --target aarch64-apple-darwin
   ```

2. Create release on GitHub manually and upload the binaries

## Troubleshooting

### GitHub Actions Workflow Not Triggering
- Ensure the tag follows the `v*` pattern (e.g., `v0.3.1`)
- Check that the workflow file is in `.github/workflows/release.yml`
- Verify you have push access to the repository

### Build Failures
- Check the MCP dependency is accessible
- Ensure all tests pass locally: `cargo test`
- Review the GitHub Actions logs for specific errors

### Docker Image Issues
- **Package not found**: Ensure the workflow completed successfully
- **Unauthorized error**: Package needs to be made public (one-time setup)
- **Platform mismatch**: APM images are Linux x64 only, use `--platform linux/amd64` when building
- **Slow builds**: Rust release builds can take 5-10 minutes per platform
- **GLIBC compatibility**: Linux binaries are built in Debian 12 (bookworm) container to ensure compatibility with Node:20 base images (GLIBC 2.36)

### Making Docker Package Public (First Time Only)
1. Wait for first release workflow to complete
2. Go to https://github.com/sunnya97/agent-process-manager/pkgs/container/apm
3. Click "Package settings" on the right
4. In "Danger Zone", change visibility to "Public"
5. All future releases will automatically be public