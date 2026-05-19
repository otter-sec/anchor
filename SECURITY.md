# Security Policy

1. Reporting security problems
2. Security Bug Bounties
3. Incident Response Process

## Reporting security problems for Anchor

**DO NOT CREATE A GITHUB ISSUE** to report a security problem.

Instead please email anchor-security@solana.org. Provide a helpful title, detailed description of the vulnerability and an exploit proof-of-concept. Speculative submissions without proof-of-concept will be closed with no further consideration.

If you haven't done so already, please **enable two-factor auth** in your GitHub account.

Expect a response as fast as possible in the advisory, typically within 72 hours.

As a general rule of thumb, we will look to these questions to evaluate eligibility:
1. Does the bug affect multiple contracts? Vulnerabilities don't have to affect multiple contracts, but a more widespread bug is generally indicative of a fundamental issue with the library, as opposed to a mistake by the developer
2. Was the bug public knowledge previously? This may mean that it's a vulnerability class for users of Anchor, but not an issue within Anchor itself
3. How complicated is the bug to trigger? The simpler and more plausible the proof of concept, the more likely it is to be a bug in the library

Regardless, if you think you have an issue, we'd like to hear about it.

For bugs that affect production code, we will pay up to X SOL tokens according to the following guidelines. This is exclusive to any bounties claimed from the protocol. In other words, reports can't double-dip.

---

If you do not receive a response in the advisory, send an email to anchor-security@solana.org with the full URL of the advisory you have created. DO NOT include attachments or provide detail sufficient for exploitation regarding the security issue in this email. **Only provide such details in the advisory**.

## Incident Response Process

In case an incident is discovered or reported, the following process will be followed to contain, respond and remediate:

### 1. Accept the new report
In response a newly reported security problem, a member of the `otter-sec/admins` group will accept the report to turn it into a draft advisory. The `otter-sec/anchor-security-incident-response` group should be added to the draft security advisory, and create a private fork of the repository (grey button towards the bottom of the page) if necessary.

If the advisory is the result of an audit finding, follow the same process as above but add the auditor's github user(s) and begin the title with "[Audit]".

If the report is out of scope, a member of the `otter-sec/admins` group will comment as such and then close the report.

### 2. Triage
Within the draft security advisory, discuss and determine the severity of the issue. If necessary, members of the `otter-sec/anchor-security-incident-response` group may add other github users to the advisory to assist. If it is determined that this is not a critical Anchor issue then the advisory should be closed and if more follow-up is required a normal Anchor public github issue should be created.

### 3. Prepare Fixes
For the affected branches, typically all three (edge, beta and stable), prepare a fix for the issue and push them to the corresponding branch in the private repository associated with the draft security advisory. There is no CI available in the private repository so you must build from source and manually verify fixes. Code review from the reporter is ideal, as well as from multiple members of the core development team.

### 4. Notify Security Group
Once an ETA is available for the fix, a member of the `otter-sec/anchor-security-incident-response` group should notify major affected parties. The teams are all over the world and it's critical to provide actionable information at the right time. Don't be the person that wakes everybody up at 2am when a fix won't be available for hours.

### 5. Ship the patch
Once the fix is accepted it may be distributed directly to developers as a patch, depending on the vulnerability.

### 6. Public Disclosure and Release
Once the fix has been deployed to major affected parties, the patches from the security advisory may be merged into the main source repository. A new official release for each affected branch should be shipped and all parties requested to upgrade as quickly as possible.

### 7. Security Advisory Bounty Accounting and Cleanup
If this issue is eligible for a bounty, prefix the title of the security advisory with one of the following, depending on the severity:

- Bounty Category: Critical: X
- Bounty Category: Medium: X
- Bounty Category: Low: X

Confirm with the reporter that they agree with the severity assessment, and discuss as required to reach a conclusion.

We currently do not use the Github workflow to publish security advisories. Once the issue and fix have been disclosed, and a bounty category is assessed if appropriate, the GitHub security advisory is no longer needed and can be closed.

## Security Bug Bounties
At its sole discretion, the Solana Foundation may offer a bounty for valid reports of Anchor vulnerabilities. Please see below for more details. The submitter is not required to provide a mitigation to qualify.

#### IMPORTANT | PLEASE NOTE
_Note: Payments will continue to be paid out in 12-month locked SOL._

#### Critical:
_Max: 1176 SOL tokens. Min: 118 SOL tokens_

* Bypassing fundamental Anchor checks, such as account ownership, discriminator, memory safety, etc.

#### Medium:
_Max: 294 SOL tokens. Min: 59 SOL tokens_

* Denial of service attacks

#### Low:
_Max: 59 SOL tokens. Min: 1 SOL tokens_

* All remaining issues
* Attacks to devex infrastructure

### Out of Scope:
The following components are out of scope for the bounty program
* Any encrypted credentials, auth tokens, etc. checked into the repo
* Bugs in dependencies. Please take them upstream!
* Attacks that require social engineering
* Any undeveloped automated tooling (scanners, etc) results. (OK with developed PoC)
* Any asset whose source code does not exist in this repository (including, but not limited to, any and all web properties not explicitly listed on this page)

### Eligibility:
* Anyone under a grant or the financial arrangement with Solana Foundation to develop or audit related tools is not eligible
* Submissions _MUST_ include an exploit proof-of-concept to be considered eligible
* The participant submitting the bug report shall follow the process outlined within this document
* Valid exploits can be eligible even if they are not successfully executed on a public cluster
* Multiple submissions for the same class of exploit are still eligible for compensation, though may be compensated at a lower rate, however these will be assessed on a case-by-case basis
* Participants must complete KYC and sign the participation agreement here when the registrations are open https://solana.org/kyc. Security exploits will still be assessed and open for submission at all times. This needs only be done prior to distribution of tokens.

### Duplicate Reports
Compensation for duplicative reports will be split among reporters with first to report taking priority using the following equation:

R: total reports
ri: report priority
bi: bounty share

bi = 2 ^ (R - ri) / ((2^R) - 1)
#### Bounty Split Examples
| total reports | priority | share  |
| ------------- | -------- | -----: |
| 1             | 1        | 100%   |
| 2             | 1        | 66.67% |
| 2             | 2        | 33.33% |
| 3             | 1        | 57.14% |
| 3             | 2        | 28.57% |
| 3             | 3        | 14.29% |
| 4             | 1        | 53.33% |
| 4             | 2        | 26.67% |
| 4             | 3        | 13.33% |
| 4             | 4        |  6.67% |
| 5             | 1        | 51.61% |
| 5             | 2        | 25.81% |
| 5             | 3        | 12.90% |
| 5             | 4        |  6.45% |
| 5             | 5        |  3.23% |

### Payment of Bug Bounties:
* Bounties are currently awarded on a rolling/weekly basis and paid out within 30 days upon receipt of an invoice.
* Bug bounties that are paid out in SOL are paid to stake accounts with a lockup expiring 12 months from the date of delivery of SOL.