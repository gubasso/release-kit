//! The engine's judgments, pure.
//!
//! What the request proposes, whether it arms, and which outcome a push
//! gets. The actions gather the facts; these
//! rules alone decide what they mean, so each is testable without a forge.

use crate::detect::Forge;
use crate::landing::manifest::{Integration, Style};

use super::adapter::Version;

/// What the request half does with the version it read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposal {
    /// Nothing remains to release, for the reason stated.
    Nothing(String),
    /// Propose this version.
    Propose(Version),
    /// Refuse, for the reason stated.
    Refuse(String),
}

/// Whether the proposal needs git-cliff's bumped version: the baseline,
/// and a final version that already released.
#[must_use]
pub const fn needs_cliff(current: Version, released: bool) -> bool {
    !current.is_candidate()
        && (released || (current.major == 0 && current.minor == 0 && current.patch == 0))
}

/// The version the request proposes.
///
/// SATISFIES release-engine:the-request-never-numbers-a-candidate
///
/// A candidate proposes nothing: only `rk lines mint` writes and strips an
/// rc number. A final version with no release proposes itself, except the
/// `0.0.0` baseline, which never releases and takes git-cliff's proposal.
/// A released final version takes git-cliff's bumped version, and a
/// prerelease proposal refuses. `cliff` is git-cliff's answer, read where
/// [`needs_cliff`] says so.
#[must_use]
pub fn propose(current: Version, released: bool, cliff: Option<&str>) -> Proposal {
    if current.is_candidate() {
        return Proposal::Nothing(format!(
            "{} is a release candidate; its line's request proposes the promoted version",
            current.tag()
        ));
    }
    if !needs_cliff(current, released) {
        return Proposal::Propose(current);
    }
    let Some(answer) = cliff else {
        return Proposal::Refuse("git-cliff proposed no version".to_owned());
    };
    let text = answer.trim().trim_start_matches('v');
    let Some(next) = Version::parse(text) else {
        return Proposal::Refuse(format!(
            "git-cliff proposed {answer:?}, which is not a final version"
        ));
    };
    if next.is_candidate() {
        return Proposal::Refuse(format!(
            "git-cliff proposed the prerelease {}; only rk lines mint numbers a candidate",
            next.tag()
        ));
    }
    if next == current {
        return Proposal::Nothing(format!(
            "no release-worthy commit follows {}",
            current.tag()
        ));
    }
    Proposal::Propose(next)
}

/// Whether the request for `base` arms itself.
///
/// SATISFIES release-engine:the-request-arms-where-the-projection-arms
///
/// The trunk style arms its request on the trunk, except on GitHub under
/// local integration, where the rendered gate job merges it. The lines
/// style and every line leave the merge to a person.
#[must_use]
pub fn arms(
    forge: Forge,
    integration: Integration,
    style: Option<Style>,
    base: &str,
    trunk: &str,
) -> bool {
    style == Some(Style::Trunk)
        && base == trunk
        && !(forge == Forge::Github && integration == Integration::Local)
}

/// What a tag read answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagAt {
    /// The forge answered that no such tag exists.
    Absent,
    /// The tag names this commit.
    At(String),
    /// The read failed, with the forge CLI's words.
    Unread(String),
}

/// The release of the version the forge holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Existing {
    /// No release of the version.
    None,
    /// An unpublished draft, which creates no tag and which nothing public
    /// reads.
    Draft,
    /// A published release.
    Published,
}

/// One push's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Release the version: no release of it exists yet.
    Release,
    /// Finish the release that exists at this commit.
    Resume,
    /// Release nothing, with success, for the reason stated.
    Skip(String),
    /// Refuse, for the reason stated, under the named recovery section.
    Refuse(String),
}

impl Outcome {
    /// The word the report uses.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Resume => "resume",
            Self::Skip(_) => "skip",
            Self::Refuse(_) => "refuse",
        }
    }
}

/// What the parent commit carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parent {
    /// The parent holds no version file, or the commit has no parent.
    Absent,
    /// The parent's version file, with the version it reads as, where it
    /// reads as one.
    Carries(Option<Version>),
}

/// What the commit itself says before the forge is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Push {
    /// The version the commit carries.
    pub version: Version,
    /// What the parent carries.
    pub parent: Parent,
    /// Whether the commit merges the engine's own request, read from the
    /// forge; it decides a final version alone.
    pub merges_request: bool,
}

/// The skip the commit alone decides, before any release or tag read.
///
/// SATISFIES release-engine:a-final-version-releases-at-its-request
///
/// The commit that introduces the version file is the landing, the
/// baseline never releases, a final version releases only at the commit
/// that merges the engine's own request, and a candidate releases only at
/// the commit that writes it, the push of `rk lines mint`.
#[must_use]
pub fn trigger(push: Push) -> Option<String> {
    let Parent::Carries(parent) = push.parent else {
        return Some("this commit introduces the version file; nothing to release".to_owned());
    };
    if push.version == super::adapter::Version::BASELINE {
        return Some("0.0.0 is the unreleased baseline; nothing to release".to_owned());
    }
    if push.version.is_candidate() {
        if parent == Some(push.version) {
            return Some(format!(
                "this commit does not write the candidate {}; nothing to release",
                push.version.tag()
            ));
        }
        return None;
    }
    if !push.merges_request {
        return Some(format!(
            "this commit merges no release request of the engine; {} releases at its request's merge",
            push.version.tag()
        ));
    }
    None
}

/// The outcome once the forge answered for the release and the tag.
///
/// SATISFIES release-engine:a-version-releases-once
///
/// A published release at another commit is an un-bump and skips. A
/// release or a tag at this commit, or an unpublished draft, resumes.
/// No release and no tag releases. A tag at another commit with no
/// published release, and a tag nobody could read, refuse.
#[must_use]
pub fn judge(head: &str, tag_name: &str, existing: Existing, tag: &TagAt) -> Outcome {
    let unread = |reason: &str| {
        Outcome::Refuse(format!(
            "the tag {tag_name} could not be read: {reason}; a tag nobody read may name another commit"
        ))
    };
    match (existing, tag) {
        (_, TagAt::Unread(reason)) => unread(reason),
        (Existing::Published, TagAt::At(at)) if at != head => Outcome::Skip(format!(
            "{tag_name} is already released at {at}; nothing to release"
        )),
        (Existing::Published, TagAt::Absent) => Outcome::Refuse(format!(
            "{tag_name} has a published release and no tag the forge answers for"
        )),
        (_, TagAt::At(at)) if at != head => Outcome::Refuse(format!(
            "{tag_name} already tags {at}, not this commit {head}, and no release of it is published"
        )),
        (Existing::None, TagAt::Absent) => Outcome::Release,
        _ => Outcome::Resume,
    }
}

#[cfg(test)]
mod tests {
    use super::{Existing, Outcome, Parent, Proposal, Push, TagAt, arms, judge, propose, trigger};
    use crate::detect::Forge;
    use crate::landing::manifest::{Integration, Style};
    use crate::release_engine::adapter::Version;

    fn v(text: &str) -> Version {
        Version::parse(text).expect("a version")
    }

    #[test]
    fn the_proposal_follows_the_candidate_shape() {
        assert!(matches!(
            propose(v("1.4.3-rc.1"), false, None),
            Proposal::Nothing(_)
        ));
        assert_eq!(
            propose(v("1.4.3"), false, None),
            Proposal::Propose(v("1.4.3"))
        );
        assert_eq!(
            propose(v("0.0.0"), false, Some("v0.1.0")),
            Proposal::Propose(v("0.1.0"))
        );
        assert_eq!(
            propose(v("1.4.3"), true, Some("v1.4.4\n")),
            Proposal::Propose(v("1.4.4"))
        );
        assert!(matches!(
            propose(v("1.4.3"), true, Some("v1.4.3")),
            Proposal::Nothing(_)
        ));
        for prerelease in ["v1.4.4-rc.1", "v1.4.4-alpha.1", "1.4.4rc1"] {
            assert!(
                matches!(
                    propose(v("1.4.3"), true, Some(prerelease)),
                    Proposal::Refuse(_)
                ),
                "{prerelease}"
            );
        }
    }

    #[test]
    fn only_the_trunk_style_on_the_trunk_arms() {
        let trunk = Some(Style::Trunk);
        assert!(arms(
            Forge::Github,
            Integration::Forge,
            trunk,
            "master",
            "master"
        ));
        assert!(arms(
            Forge::Gitlab,
            Integration::Local,
            trunk,
            "master",
            "master"
        ));
        assert!(!arms(
            Forge::Github,
            Integration::Local,
            trunk,
            "master",
            "master"
        ));
        assert!(!arms(
            Forge::Gitlab,
            Integration::Forge,
            Some(Style::Lines),
            "master",
            "master"
        ));
        assert!(!arms(
            Forge::Gitlab,
            Integration::Forge,
            trunk,
            "release/1.4",
            "master"
        ));
    }

    #[test]
    fn the_trigger_reads_the_commit_before_the_forge() {
        let push = |version: &str, parent: Option<Option<&str>>, merges| Push {
            version: v(version),
            parent: parent.map_or(Parent::Absent, |parent| Parent::Carries(parent.map(v))),
            merges_request: merges,
        };
        assert!(trigger(push("0.1.0", None, true)).is_some());
        assert!(trigger(push("0.0.0", Some(Some("0.0.0")), true)).is_some());
        assert!(trigger(push("0.1.0", Some(Some("0.0.0")), false)).is_some());
        assert!(trigger(push("0.1.0", Some(Some("0.1.0")), true)).is_none());
        assert!(trigger(push("1.4.3-rc.1", Some(Some("1.4.2")), false)).is_none());
        assert!(trigger(push("1.4.3-rc.1", Some(Some("1.4.3-rc.1")), false)).is_some());
    }

    #[test]
    fn each_release_state_gets_one_outcome() {
        let here = "aaa";
        let at = |sha: &str| TagAt::At(sha.to_owned());
        assert_eq!(
            judge(here, "v1.0.0", Existing::None, &TagAt::Absent),
            Outcome::Release
        );
        assert_eq!(
            judge(here, "v1.0.0", Existing::None, &at(here)),
            Outcome::Resume
        );
        assert_eq!(
            judge(here, "v1.0.0", Existing::Draft, &TagAt::Absent),
            Outcome::Resume
        );
        assert_eq!(
            judge(here, "v1.0.0", Existing::Published, &at(here)),
            Outcome::Resume
        );
        assert!(matches!(
            judge(here, "v1.0.0", Existing::Published, &at("bbb")),
            Outcome::Skip(_)
        ));
        assert!(matches!(
            judge(here, "v1.0.0", Existing::None, &at("bbb")),
            Outcome::Refuse(_)
        ));
        assert!(matches!(
            judge(
                here,
                "v1.0.0",
                Existing::Published,
                &TagAt::Unread("HTTP 500".into())
            ),
            Outcome::Refuse(_)
        ));
    }
}
