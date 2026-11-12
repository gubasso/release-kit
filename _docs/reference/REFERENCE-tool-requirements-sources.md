# Tool Requirements Sources

External sources behind the floors of `SPEC-tool-requirements.md`: the release of each tool that added the call `rk` makes, and which rule each fact bears on.

Verified against the listed sources on 2026-11-12.

## git, on `git hook run`

`builtin/hook.c` and `Documentation/git-hook.txt` first appear at the `v2.36.0` tag, with the synopsis `git hook run [--ignore-missing] <hook-name> [-- <hook-args>]`, and neither exists at `v2.35.0`. So git 2.36.0 is the first release that runs a repository's installed hook on request, which the integration gate needs.

- <https://github.com/git/git/blob/v2.36.0/Documentation/git-hook.txt>
- <https://github.com/git/git/blob/v2.36.0/builtin/hook.c>

Bearing: `tool-requirements:a-verb-checks-its-tools-first`.

## gh, on `gh auth status --active`

The release notes of gh 2.57.0 list "Added `--active` flag to the `gh auth status` command" (cli/cli#9520). `pkg/cmd/auth/status/status.go` declares the flag at `v2.57.0` and does not at `v2.56.0`. The flag judges only the credential the CLI would use, where the bare command fails while any stored account is broken.

- <https://github.com/cli/cli/releases/tag/v2.57.0>
- <https://github.com/cli/cli/pull/9520>
- <https://github.com/cli/cli/blob/v2.57.0/pkg/cmd/auth/status/status.go>

Bearing: `tool-requirements:a-verb-checks-its-tools-first`.

## The HTTP client, its TLS stack, and its trust store

`rk` sends its requests through ureq 3.5.2, pinned exactly because its rustls provider setter is outside its semver promise, with rustls and the ring provider, which rk hands to ureq itself, and no default feature, so no bundled root store is built in. The stack links no system OpenSSL and needs no system library; ring compiles its own C and assembly, which the single `x86_64-unknown-linux-gnu` dist target builds with the toolchain it already has. Against the tree before the change, the normal dependencies rise from 61 to 80 crates: ureq, ureq-proto, http, httparse, bytes, percent-encoding, base64, log, once_cell, utf8-zero, rustls, rustls-webpki, rustls-pki-types, ring, untrusted, subtle, getrandom 0.2, rustls-native-certs, and openssl-probe. subtle is BSD-3-Clause, which `deny.toml` now allows, and ring 0.17 takes getrandom 0.2 and windows-sys 0.52 beside the newer versions the tree uses, which `deny.toml` skips by name. reqwest, the alternative, pulls an async runtime in for seven blocking calls.

rustls-native-certs 0.8.5 loads only what `SSL_CERT_FILE` and `SSL_CERT_DIR` name where either is set (`load_native_certs`), in place of the platform store. `rk` keeps the platform store beside them. On a Unix other than macOS it reads every certificate in the system directories that openssl-probe 0.2 lists (`candidate_cert_dirs`), which no variable changes. It then adds the first bundle file that `SSL_CERT_FILE`, `NIX_SSL_CERT_FILE`, or `CURL_CA_BUNDLE` names, and each directory of `SSL_CERT_DIR` (`load_certs_from_paths`). openssl-probe is a pure-Rust search for certificate paths and links nothing.

ureq reads one proxy from `ALL_PROXY`, `HTTPS_PROXY`, or `HTTP_PROXY` for every scheme, and honors `NO_PROXY` (`Proxy::try_from_env`, `NoProxy::try_from_env`). `rk` chooses the proxy itself, for each request and each redirect, and follows the redirects itself, so each hop takes its own proxy and stays under the test seam. curl reads `no_proxy` before `NO_PROXY`, and matches a host, a domain above it, an address, and an address range in CIDR form. curl reads `http_proxy` in lowercase alone. `rk` also reads `HTTP_PROXY` after it, as the 0.9.15 decision on proxies named it.

- <https://docs.rs/ureq/3.5.2/ureq/>
- <https://github.com/algesten/ureq/blob/3.5.2/src/proxy.rs>
- <https://github.com/rustls/rustls-native-certs/blob/v/0.8.5/src/lib.rs>
- <https://curl.se/docs/manpage.html#ENVIRONMENT>
- <https://curl.se/libcurl/c/CURLOPT_NOPROXY.html>
- <https://github.com/rustls/openssl-probe/blob/0.2.2/src/lib.rs>

Bearing: `tool-requirements:rk-sends-its-own-requests`.

## The App token, signed with ring

GitHub's App authentication takes an RS256 JWT with the claims `iat`, `exp` at most ten minutes later, and `iss`, the App id, signed with the App's private key, which GitHub issues as a PKCS#1 PEM. ring 0.17 signs RSASSA-PKCS1-v1_5 with SHA-256 (`RSA_PKCS1_SHA256`) from a PKCS#1 DER key (`RsaKeyPair::from_der`) or a PKCS#8 one, and rustls-pki-types parses the PEM. The `rsa` crate is the pure-Rust alternative behind jsonwebtoken's `rust_crypto` feature. RUSTSEC-2023-0071, a timing side channel on private key operations, affects its versions before 0.10.0, and the advisory lists 0.10.0 and later as patched. The tests use `rsa` 0.10.0 to generate a test key alone. `rk` signs with ring, which it already carries for TLS, so it needs no second RSA implementation.

- <https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app>
- <https://docs.rs/ring/0.17.14/ring/rsa/struct.KeyPair.html>
- <https://rustsec.org/advisories/RUSTSEC-2023-0071.html>

Bearing: `tool-requirements:rk-sends-its-own-requests`.

## The headers hook managers write, and prek's workspace

pre-commit writes every hook from `pre_commit/resources/hook-tmpl`, whose second line reads `# File generated by pre-commit: https://pre-commit.com`. prek writes its shims from `crates/prek/src/cli/install.rs`, whose second line reads `# File generated by prek: https://github.com/j178/prek`. prek discovers every `.pre-commit-config.yaml` below its root as a project of its own. prek 0.5.10 walks the directories with `prek_ignore::walk(root, ".prekignore", ...)` and judges each child directory by the first rule that names it, from every directory above it: a `.prekignore` first, then `.ignore`, then `.gitignore` and `info/exclude`, and the global excludes last. It skips a hidden directory that no rule names, and a directory whose name starts with `{{`, ends with `}}`, and contains `cookiecutter` (`discover_fresh`). A pattern that names only a configuration file leaves its directory in the walk. git 2.54 added `git hook list`, which names each hook that git's configuration defines (`hook.<name>.event`) and names the hooks directory's own file `hook from hookdir`.

- <https://github.com/pre-commit/pre-commit/blob/v4.5.1/pre_commit/resources/hook-tmpl>
- <https://github.com/j178/prek/blob/v0.5.7/crates/prek/src/cli/install.rs>
- <https://prek.j178.dev/reference/workspace/>
- <https://github.com/j178/prek/blob/v0.5.10/crates/prek/src/workspace.rs>
- <https://github.com/j178/prek/blob/v0.5.10/crates/prek-ignore/src/walk.rs>
- <https://github.com/j178/prek/blob/v0.5.10/crates/prek-ignore/src/dir.rs>
- <https://github.com/git/git/blob/v2.54.0/Documentation/git-hook.adoc>

Bearing: `tool-requirements:the-doctor-reports-the-hook-runner`, `tool-requirements:a-nested-hook-configuration-is-reported`.
