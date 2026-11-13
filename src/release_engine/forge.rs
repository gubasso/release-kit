//! The forge reads and writes of the release engine, through `gh` and
//! `glab`, the clients `rk release status` reads through.
//!
//! rk keeps no HTTP client and holds no token: each client reads its own
//! credential from the environment the CI job sets, `GH_TOKEN` on GitHub
//! and `GITLAB_TOKEN` on GitLab. A JSON body travels on the client's
//! standard input, so no body is written to disk. A read that fails is
//! never read as an absence: only a definite not-found answers `None`. A
//! list is read whole: an exact filter where the forge offers one, and
//! every page through the client's `--paginate`, because a first page
//! proves no absence.

use std::path::PathBuf;
use std::process::Stdio;

use camino::Utf8Path;
use serde_json::{Value, json};

use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::release_state::{Api, gitlab_project};

use super::guard::TagAt;

/// One asset a release page carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// The file name.
    pub name: String,
    /// Its SHA-256 as the forge reports it, where it reports one.
    pub sha256: Option<String>,
}

/// The release of one tag.
#[derive(Debug, Clone)]
pub struct Release {
    /// The forge's handle for it: the numeric id on GitHub, the tag on
    /// GitLab.
    pub id: String,
    /// Whether it is an unpublished draft.
    pub draft: bool,
    /// Every asset or link it carries.
    pub assets: Vec<Asset>,
}

/// A forge CLI bound to one project.
#[derive(Debug, Clone)]
pub struct Client {
    cli: PathBuf,
    cwd: PathBuf,
    forge: Forge,
    repo: String,
}

impl Client {
    /// The client for `forge`, run from `target`, for project `repo`.
    ///
    /// # Errors
    ///
    /// The resolution's refusal, which names the client, where no usable
    /// binary resolves.
    pub fn resolve(forge: Forge, target: &Utf8Path, repo: &str) -> Result<Self, RkError> {
        Ok(Self {
            cli: crate::setup::context::resolve_cli(forge)?,
            cwd: target.as_std_path().to_path_buf(),
            forge,
            repo: repo.to_owned(),
        })
    }

    /// The forge.
    #[must_use]
    pub const fn forge(&self) -> Forge {
        self.forge
    }

    /// The GitLab project id, URL-encoded once.
    fn project(&self) -> String {
        gitlab_project(&self.repo)
    }

    /// Run the client with `args`, feeding `input` on its standard input.
    fn spawn(&self, args: &[&str], input: Option<&[u8]>) -> Result<std::process::Output, String> {
        let mut command = std::process::Command::new(&self.cli);
        command
            .args(args)
            .current_dir(&self.cwd)
            .env("GH_PAGER", "")
            .env("GLAB_PAGER", "")
            .env("NO_COLOR", "1")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|source| format!("the forge CLI did not run: {source}"))?;
        if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
            use std::io::Write as _;
            stdin
                .write_all(bytes)
                .map_err(|source| format!("the forge CLI did not take its input: {source}"))?;
        }
        child
            .wait_with_output()
            .map_err(|source| format!("the forge CLI did not finish: {source}"))
    }

    /// One `api` call, with a JSON body where one is given.
    #[must_use]
    pub fn api(&self, method: &str, path: &str, body: Option<&Value>) -> Api {
        self.call(method, path, body, false)
    }

    /// One `api` call; under `paginate` the client follows every page and
    /// the answer is every page's items in one array.
    fn call(&self, method: &str, path: &str, body: Option<&Value>, paginate: bool) -> Api {
        let mut args = vec!["api", "--method", method, path];
        if paginate {
            args.push("--paginate");
        }
        let input = body.map(Value::to_string);
        if input.is_some() {
            args.extend(["--input", "-"]);
        }
        let output = match self.spawn(&args, input.as_deref().map(str::as_bytes)) {
            Ok(output) => output,
            Err(reason) => return Api::Failed(reason),
        };
        if output.status.success() {
            if paginate {
                return pages(&output.stdout).map_or_else(Api::Failed, Api::Ok);
            }
            if output.stdout.iter().all(u8::is_ascii_whitespace) {
                return Api::Ok(Value::Null);
            }
            return serde_json::from_slice::<Value>(&output.stdout).map_or_else(
                |_| Api::Failed("the forge answer did not parse as JSON".to_owned()),
                Api::Ok,
            );
        }
        // `gh` renders "HTTP 404", `glab` "404 Not Found".
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("HTTP 404") || stderr.contains("404 Not Found") {
            return Api::Missing;
        }
        Api::Failed(super::last_line(&output.stderr))
    }

    /// A read whose failure refuses, because the action cannot decide
    /// without it; a definite not-found answers `None`.
    pub(crate) fn read(&self, path: &str) -> Result<Option<Value>, RkError> {
        self.read_with(path, false)
    }

    /// Every item of the list at `path`, read page by page; a definite
    /// not-found answers no item.
    ///
    /// # Errors
    ///
    /// A refusal where the forge does not answer any page.
    pub(crate) fn read_all(&self, path: &str) -> Result<Vec<Value>, RkError> {
        Ok(match self.read_with(path, true)? {
            Some(Value::Array(items)) => items,
            _ => Vec::new(),
        })
    }

    fn read_with(&self, path: &str, paginate: bool) -> Result<Option<Value>, RkError> {
        match self.call("GET", path, None, paginate) {
            Api::Ok(body) => Ok(Some(body)),
            Api::Missing => Ok(None),
            Api::Failed(reason) => Err(RkError::refusal(
                Diagnostic::new(
                    Reason::ObservationIncomplete,
                    format!("the forge did not answer {path}: {reason}"),
                )
                .expected(format!(
                    "{} authenticated for {}, with the credential the job sets",
                    self.forge.cli(),
                    self.repo
                ))
                .action("rerun the job once the forge answers")
                .target_state("unchanged"),
            )),
        }
    }

    /// A write whose failure is the action's failure, with the client's
    /// own words.
    pub(crate) fn write(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        what: &str,
    ) -> Result<Value, RkError> {
        match self.api(method, path, body) {
            Api::Ok(answer) => Ok(answer),
            Api::Missing => Err(write_failed(what, "the forge answered 404")),
            Api::Failed(reason) => Err(write_failed(what, &reason)),
        }
    }

    /// The open request whose head is `branch` against `base`, by number.
    ///
    /// # Errors
    ///
    /// A refusal where the forge does not answer.
    pub fn open_request(&self, base: &str, branch: &str) -> Result<Option<u64>, RkError> {
        // The head and source-branch filters are exact.
        let (path, head, number) = match self.forge {
            Forge::Github => (
                format!(
                    "repos/{}/pulls?state=open&head={}:{branch}&base={base}&per_page=100",
                    self.repo,
                    self.repo.split('/').next().unwrap_or_default()
                ),
                "/head/ref",
                "/number",
            ),
            Forge::Gitlab => (
                format!(
                    "projects/{}/merge_requests?state=opened&target_branch={base}&source_branch={}&per_page=100",
                    self.project(),
                    branch.replace('/', "%2F")
                ),
                "/source_branch",
                "/iid",
            ),
        };
        Ok(self
            .read_all(&path)?
            .iter()
            .find(|request| request.pointer(head).and_then(Value::as_str) == Some(branch))
            .and_then(|request| request.pointer(number)?.as_u64()))
    }

    /// Whether `sha` is the merge or squash commit of a merged request
    /// whose head is `branch` and whose base is `base`.
    ///
    /// SATISFIES release-engine:a-final-version-releases-at-its-request
    ///
    /// # Errors
    ///
    /// A refusal where the forge does not answer.
    pub fn merges_request(&self, sha: &str, base: &str, branch: &str) -> Result<bool, RkError> {
        let path = match self.forge {
            Forge::Github => format!("repos/{}/commits/{sha}/pulls?per_page=100", self.repo),
            Forge::Gitlab => format!(
                "projects/{}/repository/commits/{sha}/merge_requests?per_page=100",
                self.project()
            ),
        };
        let requests = self.read_all(&path)?;
        Ok(requests.iter().any(|request| match self.forge {
            Forge::Github => {
                !request["merged_at"].is_null()
                    && request["head"]["ref"].as_str() == Some(branch)
                    && request["base"]["ref"].as_str() == Some(base)
                    && request["merge_commit_sha"].as_str() == Some(sha)
            }
            Forge::Gitlab => {
                let fast_forward =
                    request["merge_commit_sha"].is_null() && request["sha"].as_str() == Some(sha);
                request["state"].as_str() == Some("merged")
                    && request["source_branch"].as_str() == Some(branch)
                    && request["target_branch"].as_str() == Some(base)
                    && (request["merge_commit_sha"].as_str() == Some(sha)
                        || request["squash_commit_sha"].as_str() == Some(sha)
                        || fast_forward)
            }
        }))
    }

    /// The release of `tag`, a draft included, where one exists.
    ///
    /// # Errors
    ///
    /// A refusal where the forge does not answer.
    pub fn release(&self, tag: &str) -> Result<Option<Release>, RkError> {
        match self.forge {
            Forge::Github => {
                // The tag endpoint answers no draft, so the list is read
                // whole.
                let path = format!("repos/{}/releases?per_page=100", self.repo);
                let Some(release) = self
                    .read_all(&path)?
                    .into_iter()
                    .find(|release| release["tag_name"].as_str() == Some(tag))
                else {
                    return Ok(None);
                };
                let id = release["id"].as_u64().unwrap_or_default().to_string();
                let assets = self
                    .read_all(&format!(
                        "repos/{}/releases/{id}/assets?per_page=100",
                        self.repo
                    ))?
                    .iter()
                    .filter_map(|asset| {
                        Some(Asset {
                            name: asset["name"].as_str()?.to_owned(),
                            sha256: asset["digest"]
                                .as_str()
                                .and_then(|digest| digest.strip_prefix("sha256:"))
                                .map(str::to_owned),
                        })
                    })
                    .collect();
                Ok(Some(Release {
                    id,
                    draft: release["draft"].as_bool().unwrap_or(false),
                    assets,
                }))
            }
            Forge::Gitlab => {
                let path = format!("projects/{}/releases/{tag}", self.project());
                if self.read(&path)?.is_none() {
                    return Ok(None);
                }
                let assets = self
                    .read_all(&format!("{path}/assets/links?per_page=100"))?
                    .iter()
                    .filter_map(|link| {
                        Some(Asset {
                            name: link["name"].as_str()?.to_owned(),
                            sha256: None,
                        })
                    })
                    .collect();
                Ok(Some(Release {
                    id: tag.to_owned(),
                    draft: false,
                    assets,
                }))
            }
        }
    }

    /// The commit `tag` names, in three states: present, absent, and a
    /// read that failed.
    #[must_use]
    pub fn tag(&self, tag: &str) -> TagAt {
        let path = match self.forge {
            Forge::Github => format!("repos/{}/git/ref/tags/{tag}", self.repo),
            Forge::Gitlab => format!("projects/{}/repository/tags/{tag}", self.project()),
        };
        let body = match self.api("GET", &path, None) {
            Api::Ok(body) => body,
            Api::Missing => return TagAt::Absent,
            Api::Failed(reason) => return TagAt::Unread(reason),
        };
        let commit = match self.forge {
            Forge::Gitlab => body["commit"]["id"].as_str().map(str::to_owned),
            Forge::Github if body["object"]["type"].as_str() == Some("tag") => {
                // An annotated tag names a tag object, which names the
                // commit.
                let Some(object) = body["object"]["sha"].as_str() else {
                    return TagAt::Unread("the tag names no object".to_owned());
                };
                match self.api(
                    "GET",
                    &format!("repos/{}/git/tags/{object}", self.repo),
                    None,
                ) {
                    Api::Ok(tag) => tag["object"]["sha"].as_str().map(str::to_owned),
                    Api::Missing => None,
                    Api::Failed(reason) => return TagAt::Unread(reason),
                }
            }
            Forge::Github => body["object"]["sha"].as_str().map(str::to_owned),
        };
        commit.map_or_else(
            || TagAt::Unread("the tag answers no commit".to_owned()),
            TagAt::At,
        )
    }

    /// Every file the GitLab generic package `release` holds at `version`,
    /// with its SHA-256.
    ///
    /// # Errors
    ///
    /// A refusal where the forge does not answer.
    pub fn package_files(&self, version: &str) -> Result<Vec<Asset>, RkError> {
        let path = format!(
            "projects/{}/packages?package_type=generic&package_name=release&package_version={version}&per_page=100",
            self.project()
        );
        // The name filter is fuzzy, so the exact package is selected.
        let id = self.read_all(&path)?.iter().find_map(|package| {
            (package["package_type"].as_str() == Some("generic")
                && package["name"].as_str() == Some("release")
                && package["version"].as_str() == Some(version))
            .then(|| package["id"].as_u64())
            .flatten()
        });
        let Some(id) = id else {
            return Ok(Vec::new());
        };
        let path = format!(
            "projects/{}/packages/{id}/package_files?per_page=100",
            self.project()
        );
        Ok(self
            .read_all(&path)?
            .iter()
            .filter_map(|file| {
                Some(Asset {
                    name: file["file_name"].as_str()?.to_owned(),
                    sha256: file["file_sha256"].as_str().map(str::to_owned),
                })
            })
            .collect())
    }

    /// The name and the email of the account the client's credential
    /// names.
    ///
    /// SATISFIES bot-identity:the-bot-identity-reaches-one-project
    ///
    /// GitHub answers the App's bot account as the viewer of its
    /// installation token, and its email is the account's noreply
    /// address. GitLab answers the token's user, and a user with no public
    /// email takes its username's noreply address on the instance.
    ///
    /// # Errors
    ///
    /// A refusal where the forge does not answer.
    pub fn identity(&self) -> Result<(String, String), RkError> {
        let unread = || {
            RkError::refusal(
                Diagnostic::new(
                    Reason::ForgeAuthentication,
                    "the forge names no account for the job's credential",
                )
                .expected("the bot credential the release job sets")
                .target_state("unchanged"),
            )
        };
        match self.forge {
            Forge::Github => {
                let output = self
                    .spawn(
                        &[
                            "api",
                            "graphql",
                            "-f",
                            "query=query { viewer { login databaseId } }",
                        ],
                        None,
                    )
                    .map_err(|reason| write_failed("the bot identity read", &reason))?;
                let body: Value = serde_json::from_slice(&output.stdout).map_err(|_| unread())?;
                let login = body["data"]["viewer"]["login"]
                    .as_str()
                    .ok_or_else(unread)?;
                let id = body["data"]["viewer"]["databaseId"]
                    .as_u64()
                    .ok_or_else(unread)?;
                Ok((
                    login.to_owned(),
                    format!("{id}+{login}@users.noreply.github.com"),
                ))
            }
            Forge::Gitlab => {
                let user = self.read("user")?.ok_or_else(unread)?;
                let name = user["name"].as_str().ok_or_else(unread)?.to_owned();
                let email =
                    if let Some(email) = user["email"].as_str().filter(|email| !email.is_empty()) {
                        email.to_owned()
                    } else {
                        let host = std::env::var("CI_SERVER_HOST")
                            .unwrap_or_else(|_| "gitlab.com".to_owned());
                        format!(
                            "{}@noreply.{host}",
                            user["username"].as_str().ok_or_else(unread)?
                        )
                    };
                Ok((name, email))
            }
        }
    }

    /// Open the request from `branch` into `base`.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn create_request(
        &self,
        base: &str,
        branch: &str,
        title: &str,
        body: &str,
    ) -> Result<u64, RkError> {
        // SATISFIES release-engine:one-request-per-base
        let answer = match self.forge {
            Forge::Github => self.write(
                "POST",
                &format!("repos/{}/pulls", self.repo),
                Some(&json!({"title": title, "head": branch, "base": base, "body": body})),
                "the release request create",
            )?,
            Forge::Gitlab => self.write(
                "POST",
                &format!("projects/{}/merge_requests", self.project()),
                Some(&json!({
                    "source_branch": branch, "target_branch": base, "title": title,
                    "description": body, "remove_source_branch": true
                })),
                "the release request create",
            )?,
        };
        answer["number"]
            .as_u64()
            .or_else(|| answer["iid"].as_u64())
            .ok_or_else(|| write_failed("the release request create", "the forge named no number"))
    }

    /// Edit the request's title and description in place.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn edit_request(&self, number: u64, title: &str, body: &str) -> Result<(), RkError> {
        // SATISFIES release-engine:one-request-per-base
        match self.forge {
            Forge::Github => self.write(
                "PATCH",
                &format!("repos/{}/pulls/{number}", self.repo),
                Some(&json!({"title": title, "body": body})),
                "the release request edit",
            ),
            Forge::Gitlab => self.write(
                "PUT",
                &format!("projects/{}/merge_requests/{number}", self.project()),
                Some(&json!({"title": title, "description": body})),
                "the release request edit",
            ),
        }
        .map(drop)
    }

    /// Close a request that has nothing left to release.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn close_request(&self, number: u64) -> Result<(), RkError> {
        // SATISFIES release-engine:one-request-per-base
        match self.forge {
            Forge::Github => self.write(
                "PATCH",
                &format!("repos/{}/pulls/{number}", self.repo),
                Some(&json!({"state": "closed"})),
                "the stale release request close",
            ),
            Forge::Gitlab => self.write(
                "PUT",
                &format!("projects/{}/merge_requests/{number}", self.project()),
                Some(&json!({"state_event": "close"})),
                "the stale release request close",
            ),
        }
        .map(drop)
    }

    /// Create an unpublished draft release of `tag` at `sha`, which
    /// creates no tag, and answer its id.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn create_draft(
        &self,
        tag: &str,
        sha: &str,
        notes: &str,
        candidate: bool,
    ) -> Result<String, RkError> {
        // SATISFIES release-engine:the-tag-is-created-last
        // SATISFIES release-engine:a-candidate-is-a-prerelease
        let answer = self.write(
            "POST",
            &format!("repos/{}/releases", self.repo),
            Some(&json!({
                "tag_name": tag, "target_commitish": sha, "name": tag, "body": notes,
                "draft": true, "prerelease": candidate
            })),
            "the draft release create",
        )?;
        answer["id"]
            .as_u64()
            .map(|id| id.to_string())
            .ok_or_else(|| write_failed("the draft release create", "the forge named no id"))
    }

    /// Upload files to the GitHub release of `tag`, replacing a same-name
    /// asset where `clobber` says so.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn upload(
        &self,
        tag: &str,
        files: &[camino::Utf8PathBuf],
        clobber: bool,
    ) -> Result<(), RkError> {
        // SATISFIES release-engine:every-step-converges
        if files.is_empty() {
            return Ok(());
        }
        let mut args = vec!["release", "upload", tag];
        args.extend(files.iter().map(|file| file.as_str()));
        if clobber {
            args.push("--clobber");
        }
        args.extend(["--repo", self.repo.as_str()]);
        let output = self
            .spawn(&args, None)
            .map_err(|reason| write_failed("the asset upload", &reason))?;
        if output.status.success() {
            return Ok(());
        }
        Err(write_failed(
            "the asset upload",
            &super::last_line(&output.stderr),
        ))
    }

    /// Publish a draft at `sha`; the publication creates the tag there.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn publish_draft(&self, id: &str, sha: &str, candidate: bool) -> Result<(), RkError> {
        // SATISFIES release-engine:the-tag-is-created-last
        // SATISFIES release-engine:a-candidate-is-a-prerelease
        self.write(
            "PATCH",
            &format!("repos/{}/releases/{id}", self.repo),
            Some(&json!({
                "draft": false, "target_commitish": sha,
                "make_latest": if candidate { "false" } else { "true" }
            })),
            "the draft release publish",
        )
        .map(drop)
    }

    /// The URL a GitLab generic package file is served from.
    #[must_use]
    pub fn package_url(&self, version: &str, name: &str) -> String {
        let api = std::env::var("CI_API_V4_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| "https://gitlab.com/api/v4".to_owned());
        format!(
            "{api}/projects/{}/packages/generic/release/{version}/{name}",
            self.project()
        )
    }

    /// Upload one file to the GitLab generic package `release` at
    /// `version`.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn upload_package(&self, version: &str, file: &Utf8Path) -> Result<(), RkError> {
        // SATISFIES release-engine:every-step-converges
        let name = super::file_name(file);
        let path = format!(
            "projects/{}/packages/generic/release/{version}/{name}",
            self.project()
        );
        let bytes = std::fs::read(file).map_err(RkError::Io)?;
        let output = self
            .spawn(
                &[
                    "api",
                    "--method",
                    "PUT",
                    &path,
                    "--header",
                    "Content-Type: application/octet-stream",
                    "--input",
                    "-",
                ],
                Some(&bytes),
            )
            .map_err(|reason| write_failed("the package upload", &reason))?;
        if output.status.success() {
            return Ok(());
        }
        Err(write_failed(
            &format!("the package upload of {name}"),
            &super::last_line(&output.stderr),
        ))
    }

    /// Create the GitLab release of `tag` at `sha` with every link, in one
    /// request; the release create creates the tag.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn create_release(
        &self,
        tag: &str,
        sha: &str,
        description: &str,
        links: &[(String, String)],
    ) -> Result<(), RkError> {
        // SATISFIES release-engine:the-tag-is-created-last
        let links: Vec<Value> = links
            .iter()
            .map(|(name, url)| json!({"name": name, "url": url}))
            .collect();
        self.write(
            "POST",
            &format!("projects/{}/releases", self.project()),
            Some(&json!({
                "tag_name": tag, "ref": sha, "name": tag, "description": description,
                "assets": {"links": links}
            })),
            "the release create",
        )
        .map(drop)
    }

    /// Add one link to the GitLab release of `tag`.
    ///
    /// # Errors
    ///
    /// The forge's refusal.
    pub fn add_link(&self, tag: &str, name: &str, url: &str) -> Result<(), RkError> {
        // SATISFIES release-engine:every-step-converges
        self.write(
            "POST",
            &format!("projects/{}/releases/{tag}/assets/links", self.project()),
            Some(&json!({"name": name, "url": url})),
            "the release link add",
        )
        .map(drop)
    }
}

/// The items of every page a paginated read printed. `gh` prints one JSON
/// array per page, and `glab` one array of every page's items, so a stream
/// of arrays is merged here.
fn pages(stdout: &[u8]) -> Result<Value, String> {
    let mut items = Vec::new();
    for page in serde_json::Deserializer::from_slice(stdout).into_iter::<Value>() {
        match page {
            Ok(Value::Array(page)) => items.extend(page),
            Ok(_) => return Err("a page of the forge answer is not a JSON array".to_owned()),
            Err(_) => return Err("the forge answer did not parse as JSON".to_owned()),
        }
    }
    Ok(Value::Array(items))
}

/// A forge write that failed, with the client's own words.
fn write_failed(what: &str, reason: &str) -> RkError {
    RkError::subprocess(
        Diagnostic::new(Reason::SubprocessFailed, format!("{what} failed: {reason}"))
            .expected(format!("{what} the forge accepts"))
            .action("rerun the job: every step keeps what already exists")
            .target_state("every write before this one stands"),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::pages;

    #[test]
    fn every_page_of_a_list_merges_into_one() {
        assert_eq!(pages(b"[1,2][3]"), Ok(json!([1, 2, 3])));
        assert_eq!(pages(b"[1]\n[2]\n"), Ok(json!([1, 2])));
        assert_eq!(pages(b"[1,2,3]"), Ok(json!([1, 2, 3])));
        assert_eq!(pages(b""), Ok(json!([])));
        assert!(pages(b"{}").is_err());
        assert!(pages(b"[1").is_err());
    }
}
