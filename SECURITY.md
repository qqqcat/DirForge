# Security Policy

DirOtter works with local filesystem metadata and cleanup operations. Security reports that involve data loss, unsafe deletion, path traversal, permission boundaries, or platform-specific trash/recycle-bin behavior are treated as high priority.

## Supported Versions

Security fixes are currently accepted for the latest release line and the current `main` branch.

Because the project is still early, older release lines may not receive separate patch releases unless a maintainer explicitly marks them as supported.

## Reporting a Vulnerability

Please report security or data-loss issues privately before opening a public issue when possible.

Use GitHub private vulnerability reporting if it is enabled for this repository. If it is not available, contact a maintainer privately through the repository owner account and include enough detail to reproduce the issue.

Useful report details include:

- operating system and version
- DirOtter version or commit
- affected path type, such as normal directory, symlink, junction, mount point, recycle bin, or trash location
- whether the operation was scan-only, review, recycle/trash, or permanent delete
- expected behavior
- actual behavior
- minimal reproduction steps
- logs, screenshots, or test fixtures when safe to share

Do not include private file contents, credentials, tokens, or sensitive personal paths unless they are required to explain the issue. Redact where possible.

## Sensitive Areas

Reports are especially important in these areas:

- unsafe deletion or irreversible cleanup behavior
- incorrect high-risk path classification
- symlink, junction, mount-point, or traversal mistakes
- permission boundary mistakes
- recycle-bin or trash failure handling
- cleanup recommendations that target protected, system, or user-critical paths
- UI state that says an operation is safe, complete, or idle when the runtime state disagrees
- packaged release artifacts that differ from source safety behavior

## Response Timeline

Maintainers aim to acknowledge security reports within 7 days.

For confirmed high-impact data-loss or unsafe-delete issues, maintainers aim to provide a mitigation plan or fix target within 14 days of acknowledgement.

These timelines are targets, not guarantees. If a report needs more investigation, maintainers should provide status updates until the issue is resolved or downgraded.

## Disclosure Policy

Please avoid public disclosure until maintainers have had a reasonable chance to investigate and prepare a fix or mitigation.

After a fix is available, the project may publish a security advisory, release note, or public issue summary depending on severity and user impact. Credit will be given to reporters who want public acknowledgement.

If maintainers do not respond within a reasonable time, coordinated disclosure is still preferred over releasing exploit details without notice.
