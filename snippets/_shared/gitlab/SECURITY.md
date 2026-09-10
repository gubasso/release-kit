# Security policy

## Report a vulnerability

Report a suspected vulnerability as a confidential issue in the RK_REPO project on the GitLab instance where this repository is hosted. Sign in and open this project's new issue form. Follow [GitLab's confidential issue instructions](https://docs.gitlab.com/user/project/issues/confidential_issues/) and verify that confidentiality is selected before submitting any sensitive details. Confidential submission depends on your access to the project. If the form is unavailable, use an existing private conversation with a maintainer to arrange access.

Do not disclose a vulnerability in a public issue, merge request, comment, or commit. The confidential issue is the reporting channel; project members with sufficient permissions can read it.

Include the affected release, component, required configuration, reproduction steps or a minimal proof of concept, and observed security impact. Remove credentials and personal information from attachments. A dependency version and a CVE identifier alone do not establish impact; explain how the vulnerable behavior is reached through this project.

## Supported releases and disclosure

Start with the latest published release. Older releases receive fixes only where the project documents a maintained release line. A fix ships as a new version; withdrawing a version contains exposure and does not repair existing installations.

Reports are handled on a best-effort basis. Coordinate public disclosure while maintainers investigate and prepare a fix. This policy commits to no response or disclosure deadline.
