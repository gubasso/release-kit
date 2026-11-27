---
upstream: https://gitlab.com/gitlab-org/gitlab/-/issues/584488
affects: the projects API of GitLab 18.1 to 18.9, and of every GitLab for a reader below Maintainer, which does not show protect_merge_request_pipelines
state: monitoring
filing: deferred
checked: 2026-11-26
workaround: none. The operator ordered on 2026-11-26 to keep the block in the release and record the case. The ci-permissions step and the protections check stay unsatisfied on such an instance or for such a reader, even after a person clears the setting by hand.
retire_when: rk can record that a person cleared the setting by hand, or GITLAB_VERSION_FLOOR in src/setup/observe.rs reaches 18.10 and GitLab shows protect_merge_request_pipelines to a reader below Maintainer.
---

# GitLab hides the merge request pipeline setting from the projects API before 18.10

## Symptom

A project releases through a self-managed GitLab at 18.2 to 18.9. Its operator runs `rk setup check`. The `ci-permissions` step fails with this text:

```text
FAIL this GitLab does not show protect_merge_request_pipelines to this reader: the projects API returns it from GitLab 18.10, and only to a Maintainer or above.
```

The operator clears "Allow merge request pipelines to access protected variables and runners" under Settings > CI/CD > Variables by hand. The next `rk setup check` fails with the same text. `rk setup --apply` stays blocked at the same step. A reader below Maintainer meets the same failure on every GitLab version from 18.1.

## How it works

1. GitLab 18.1 added the setting, in merge request 188008. From 18.1, a new project lets its merge request pipelines reach the protected variables and runners. Such a pipeline runs the code of its source branch.
2. The projects API returns the setting as `protect_merge_request_pipelines` only from GitLab 18.10, and only to a Maintainer or above. `lib/api/entities/project.rb` states both limits.
3. `setup/gitlab/ci-permissions` reads the project. The answer carries no attribute. The step reads the instance version, for example `18.6.2-ee`. The version is at 18.1 or later, so the step cannot tell whether the access is on. It writes nothing and fails.
4. A person clears the setting in the web interface. The projects API still returns no attribute, so the step reads the same answer and fails again. rk has no record where the person can state the clearance.

The cause: the forge does not show the setting that rk must prove. A triager first reads the failure as a defect of rk. That reading is wrong, because rk fails closed by `forge-setup:a-check-reports-what-the-forge-enforces`. A pass on an unread setting can leave the protected release secrets open to a merge request pipeline. The smallest fix is a committed answer that records the clearance by hand, which `rk setup check` then reports as an operator statement.

## Signal

The text `this GitLab does not show protect_merge_request_pipelines to this reader` in the output of `rk setup check` or `rk setup --apply`, on a GitLab below 18.10 or for a reader below Maintainer.

## Workaround

None. The tree carries no change for this case. On GitLab 18.10 or later, read the project as a Maintainer, and the step passes once the setting reads `false`.
