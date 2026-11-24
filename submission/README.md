# Marketplace submission package

Destination: https://github.com/omacom/omarchy-plugin-marketplace
Repository: https://github.com/7ang0n1n3/omatorsurf
Title: `[Plugin]: Omatorsurf`
Body: [issue.md](issue.md)
Category: System. Tags: Bar, Quickshell, Security.

The official submit-plugin.yml form and SUBMISSION.md were checked on 2026-11-24:
https://github.com/omacom/omarchy-plugin-marketplace/blob/main/.github/ISSUE_TEMPLATE/submit-plugin.yml
https://github.com/omacom/omarchy-plugin-marketplace/blob/main/SUBMISSION.md

The root manifest references quickshell-plugin/BarWidget.qml. The nested manifest
is the same plugin in the flat layout used by the manual installer, not a second
listing. Native backend/build dependencies and root helper setup require manual
installation review; the maintainer determines the final listing classification.

Before posting:

1. Complete the outstanding checks in VALIDATION.md and commit/push the intended
   source. Record its full SHA there; the current base SHA is not this new candidate.
2. Verify public repository access and search existing marketplace issues and the
   catalog for the repository and permanent ID to avoid a duplicate request.
3. Review every issue checklist statement with the owner. Check each box only
   after confirmation. The unchecked draft intentionally cannot pass intake yet.
4. Recheck the live form and show the final exact body before posting. Preparation
   has not published an issue, tag, release or marketplace entry.

After owner confirmation, the posting command is:

```bash
gh issue create --repo omacom/omarchy-plugin-marketplace \
  --title "[Plugin]: Omatorsurf" --body-file submission/issue.md
```

A root preview image is optional for marketplace intake. None is fabricated here.
Before a visual release, add a reproducible screenshot using fictional IP/status
data and record its provenance. Do not publish a desktop capture with private data.

If automated review reports problems, update the existing request following the
maintainer's directions. Listing approval is distinct from a security audit.
