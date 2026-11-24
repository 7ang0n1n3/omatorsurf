# Submission validation record

Prepared 2026-11-24. Plugin 1.0.2; backend 1.0.0.
Base commit: `ccd2f5e84ba736b4ccd8d88a27fc520858263a85` plus the uncommitted
submission preparation changes. Final candidate SHA: pending commit and push.
No tag, release or marketplace request was created.

## Checks performed

- `omarchy plugin validate .`: exit 0, VALID against the installed Omarchy validator.
- Portable skill manifest/path validation: exit 0.
- `tests/run`: manifest equivalence, entry point layout, Python AST parsing,
  shell syntax and the backend Rust unit suite: exit 0, 54 tests passed.
  The first sandboxed run could not unpack the Cargo cache; the authorized rerun
  completed successfully.
- Advisory security scan: manifest valid, outcome `needs-fixes` because the
  existing README clone/build instructions use mutable upstream HEAD rather than
  a detached full-commit checkout. This finding remains unresolved; finalize an
  immutable source candidate and document installation of that reviewed revision.
- Release preflight: NOT READY. The working tree contains these uncommitted
  changes and the source-installation finding above remains. The initial missing
  standard removal documentation was corrected with its cleanup limitations.
- Expected review capabilities: privileged operations, package installation,
  source builds, service management, installer scripts and QML processes.
  The working-directory scanner also inspected ignored backup/ and target/
  contents. Those are not tracked distribution files. Re-run against the final
  clean public checkout to bind review to the actual distributed payload.

The installed validator result is compatibility evidence, not a security audit.
The host Omarchy installation is not a Git checkout, so an exact host source SHA
was not obtained. No broader Omarchy version/architecture support is claimed.

## Pending evidence before release/submission

- Final public commit SHA, clean checkout validation, green hosted CI, public
  repository access and a duplicate ID/repository search in the marketplace.
- Owner confirmation of all five exact checklist statements in issue.md.
- Review of the complete committed capability/security baseline by maintainers.
- Fresh installation, upgrade preserving layout/configuration, plugin-only and
  full removal on a disposable host, including cancellation/failure recovery.
- Live root-entry-point loading, horizontal/vertical bars, popup keyboard/dismissal,
  multi-monitor behavior, theme reload and disabled/unknown/degraded states.
- Privileged widget Start/Stop/NEWNYM, authentication cancellation and recovery.
- Reboot and suspend/resume coverage before making persistence claims.
- Optional marketplace preview: reproducible image with fictional data and
  provenance. No screenshot or artwork is supplied in this preparation.
- Omakit submission checks were not run. They do not replace official review.

Earlier README statements about live widget/status-publisher observations are
historical project evidence, not new testing of this packaging candidate. Do not
mark these pending journeys as passed based on static checks or old observations.
