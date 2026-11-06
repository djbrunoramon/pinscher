# Releasing Pinscher

1. Update the version in `Cargo.toml` and refresh `Cargo.lock` if needed.
2. Update `CHANGELOG.md` with the release notes.
3. Commit those changes and create an annotated version tag, for example:
   `git tag -a vX.Y.Z -m "Pinscher vX.Y.Z"`.
4. Push the tag to the main repository. The release workflow packages the app and creates the
   GitHub release with generated notes.

## GitHub secrets

Configure these repository secrets for a signed and notarized release:

- `MACOS_CERTIFICATE_P12`: base64-encoded Developer ID certificate export.
- `MACOS_CERTIFICATE_PASSWORD`: password for the certificate export.
- `PINSCHER_SIGN_IDENTITY`: Developer ID signing identity.
- `APPLE_ID`, `APPLE_TEAM_ID`, and `APPLE_APP_PASSWORD`: credentials for `notarytool`.

When using a pre-configured `notarytool` profile locally, set `PINSCHER_NOTARY_PROFILE` instead
of the Apple credential trio. Without signing and notarization credentials, the workflow produces
an ad-hoc signed release and clearly skips notarization.
