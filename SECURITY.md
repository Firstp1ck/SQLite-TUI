# Security Policy

This document describes how to report security vulnerabilities in sqlite-editor and which versions receive security updates.

## Scope

This policy applies to the sqlite-editor project itself (the TUI binary and its source code). Vulnerabilities found in third‑party dependencies should generally be reported upstream to those projects. We will track and update our dependencies as fixes become available.

## Supported Versions

We aim to keep users secure while minimizing maintenance burden.

| Version                      | Security Fixes |
| --------------------------- | -------------- |
| main (unreleased)           | Yes            |
| Latest release              | Yes            |
| Previous minor release      | Critical only  |
| Older releases              | No             |

Notes:
- “Latest release” means the most recent tagged release on this repository.
- “Previous minor release” may receive backported fixes for high/critical issues when feasible.
- If you cannot upgrade from an unsupported version, please detail your constraints in your report so we can suggest mitigations.

## Reporting a Vulnerability

Please use private disclosure. Do not open public issues for security reports.

Preferred: GitHub private advisory
- Navigate to the repository’s Security tab → “Report a vulnerability”.
- Provide a clear description with:
  - Affected version(s) and your OS/environment
  - Steps to reproduce and a minimal proof of concept
  - Expected vs. actual behavior
  - Impact assessment (e.g., data loss, RCE, privilege escalation)
  - Any known workarounds or mitigations

If you cannot use GitHub’s private advisory workflow:
- Contact the maintainers through any listed private contact in the repository (e.g., email in commit metadata or project profile).
- If no private contact is available, create a new issue with only a brief note requesting a private channel (do not include exploit details), and we will reach out.

## Our Process and Response Targets

We will make best efforts to follow these targets:
- Acknowledgment: within 3 business days
- Initial assessment and severity triage: within 7 days
- Fix or mitigation plan for high/critical issues: within 30 days (or communicate timeline)
- Coordinated disclosure: we will publish a security advisory and a patched release; we will credit reporters who wish to be acknowledged

Severity is assessed using common criteria (e.g., CVSS) and practical exploitability in typical deployments.

## Responsible Disclosure

Please allow us reasonable time to investigate and release a fix before public disclosure. We are committed to:
- Communicating status updates during triage and remediation
- Publishing a clear advisory with affected versions, impact, and upgrade/mitigation instructions
- Crediting reporters (unless anonymity is requested)

## Safe Harbor

We support good-faith security research:
- As long as you avoid privacy violations, service degradation, or data destruction
- Limit testing to your own environments and data
- Do not exploit beyond what is necessary to demonstrate the issue
- Act in accordance with applicable laws

Thank you for helping keep sqlite-editor and its users secure.