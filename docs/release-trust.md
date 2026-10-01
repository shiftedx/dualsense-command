# Release trust

**0.5.1 is the current unsigned Windows installer release.** Filenames identify
unsigned MSIs; Windows may show trust prompts. Future stable releases require
a certificate and verified signatures. Local unsigned packaging uses `-Unsigned`. See [open validation gates](production-readiness-plan.md).

| Artifact | Choose when |
| --- | --- |
| Standard | Normal Windows use; no broker bundled |
| Bridge | Experimental non-Steam forwarding; self-contained .NET |
| Bridge framework-dependent | Bridge testing with the matching x64 .NET runtime installed |
| Linux archive | Native Linux beta testing |

## Install or update

Quit the running tray app, then install the Standard MSI. Profiles and settings
are retained. Leave **Start with Windows** unchecked: local MSI logs and native
registry reads disagree, so startup registration/removal is not verified.

## Cut a release

1. Align package/crate/lockfile/MSI versions and add an exact tag section to `CHANGELOG.md`.
2. Run `npm run check`, `npm run check:notices`, and
   `node tools/check-release.mjs --tag v<version> --distribution`.
3. Merge reviewed changes, then tag the tested commit. CI builds all flavors,
   preserves notices, verifies the pinned HIDMaestro archive, and publishes
   SHA256 checksums. Record validation limits.
4. Verify the download against the corresponding checksum file:

```powershell
Get-FileHash .\<artifact> -Algorithm SHA256
Get-Content .\SHA256SUMS-windows.txt
```

## Signing

Set repository secrets `DSCC_SIGNING_PFX_BASE64` and `DSCC_SIGNING_PASSWORD`.
CI stores the PFX under runner temporary storage, signs with SHA256 and an
RFC3161 timestamp, verifies Authenticode, then deletes the temporary key in
`finally`. Never commit certificates, keys, captures, or generated artifacts.
See [SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/using-signtool-to-sign-a-file).

Regenerate `DEPENDENCY_LICENSES.txt` with `npm run generate:notices` after locked
updates. Preserve license texts and source links; self-contained Bridge also
requires exact restored runtime notices. Do not add an updater until signing
and rollback are verified.
