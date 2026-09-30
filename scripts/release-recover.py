"""One bounded recovery for an already-built, empty v0.3.1 draft; no tag creation or deletion."""
import json
import os
from pathlib import Path
import tempfile
import time
import release as r

SOURCE = "45636787d93261eed361762b27bd37a5dbb34841"
CORE = "f0be20778e7b5272c8bc7e6fd2ffe539e7e95da3"
TAG = "v0.3.1"
DRAFT = 399987295
RUN = 36707894610
ARTIFACT = 11094220457
DIGEST = "sha256:adb0c8a002f5fe368c220af63475a2ed9770ffeaa6602527b2fc6280e36daa3e"
BRANCH = "release/recover-v0.3.1-399987295"


def validate_draft(value, empty=False):
    r.require(value.get("id") == DRAFT and value.get("tag_name") == TAG
              and value.get("target_commitish") == SOURCE and value.get("draft") is True
              and value.get("author", {}).get("login") == "github-actions[bot]",
              "Unexpected draft identity, source, author or state")
    if empty:
        r.require(value.get("assets") == [], "Existing assets must be reviewed; never overwrite")


def unique_draft():
    # New drafts may not be visible in list results immediately. Never accept another ID.
    for attempt in range(4):
        values = r.pages(f"repos/{r.EDITOR}/releases?per_page=100")
        matches = [x for x in values if x.get("tag_name") == TAG]
        if len(matches) == 1 and matches[0].get("id") == DRAFT:
            return
        r.require(not matches and attempt < 3, "Draft list has another ID, duplicates or remains unavailable")
        validate_draft(r.api(f"repos/{r.EDITOR}/releases/{DRAFT}"))
        time.sleep(2 ** attempt)


def validate_pair(pair):
    r.require(pair.get("schema_version") == 1 and pair.get("tag") == TAG, "Wrong manifest")
    r.require(pair.get("worldedit") == {"repository": r.EDITOR, "sha": SOURCE, "version": "0.3.1"}
              and pair.get("worldline") == {"repository": r.CORE, "sha": CORE, "version": "0.3.0"},
              "Artifacts do not belong to the frozen source pair")


def gate(pair, empty=False):
    validate_pair(pair)
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    r.require(os.environ.get("GITHUB_EVENT_NAME") == "create"
              and os.environ.get("GITHUB_REPOSITORY") == r.EDITOR
              and event.get("repository", {}).get("full_name") == r.EDITOR
              and event.get("ref_type") == "branch" and event.get("ref") == BRANCH
              and os.environ.get("GITHUB_REF") == "refs/heads/" + BRANCH,
              "Only the explicit recovery branch-create event is allowed")
    recovery_sha = os.environ.get("GITHUB_SHA", "")
    r.require(r.SHA.fullmatch(recovery_sha)
              and r.api(f"repos/{r.EDITOR}/git/ref/heads/{BRANCH}")["object"]["sha"] == recovery_sha,
              "Recovery branch moved")
    r.require(r.main_sha(r.EDITOR) == SOURCE and r.main_sha(r.CORE) == CORE, "Main pair changed")
    tag = r.api(f"repos/{r.EDITOR}/git/ref/tags/{TAG}")["object"]
    r.require(tag.get("type") == "commit" and tag.get("sha") == SOURCE, "Tag changed")
    original_branch = r.api(f"repos/{r.EDITOR}/git/ref/heads/release/{TAG}")["object"]
    r.require(original_branch.get("sha") == SOURCE, "Original release branch changed")
    for repo, sha in [(r.EDITOR, SOURCE), (r.CORE, CORE)]:
        r.require(not [x for x in r.pages(f"repos/{repo}/issues?state=open&per_page=100")
                       if "pull_request" not in x], "Open issues remain")
        r.ci(repo, sha)
    artifact = r.api(f"repos/{r.EDITOR}/actions/artifacts/{ARTIFACT}")
    r.require(artifact.get("id") == ARTIFACT and artifact.get("name") == "release-assets"
              and artifact.get("digest") == DIGEST and artifact.get("expired") is False
              and artifact.get("workflow_run", {}).get("id") == RUN
              and artifact["workflow_run"].get("head_sha") == SOURCE,
              "Artifact origin or digest changed")
    jobs = r.pages(f"repos/{r.EDITOR}/actions/runs/{RUN}/jobs?per_page=100", collection="jobs")
    for name in ["gate", "build"]:
        matches = [j for j in jobs if j.get("name") == name]
        r.require(len(matches) == 1 and matches[0].get("conclusion") == "success", "Original build did not pass")
    validate_draft(r.api(f"repos/{r.EDITOR}/releases/{DRAFT}"), empty)
    unique_draft()
    r.require(r.main_sha(r.EDITOR) == SOURCE and r.main_sha(r.CORE) == CORE, "Main changed during gate")


def recover(root):
    pair = json.loads((root / "release-pair.json").read_text(encoding="utf-8"))
    r.verify_assets(root, pair)
    gate(pair, empty=True)
    r.gh("release", "upload", TAG, *[str(root / name) for name in sorted(r.ASSETS)], "--repo", r.EDITOR)
    draft = r.api(f"repos/{r.EDITOR}/releases/{DRAFT}")
    validate_draft(draft)
    assets = r.pages(f"repos/{r.EDITOR}/releases/{DRAFT}/assets?per_page=100")
    r.require(len(assets) == len(r.ASSETS) and {a["name"] for a in assets} == r.ASSETS, "Unexpected assets")
    for asset in assets:
        r.require(asset.get("state") == "uploaded" and asset.get("size") == (root / asset["name"]).stat().st_size,
                  "Asset not uploaded completely")
    with tempfile.TemporaryDirectory() as directory:
        r.gh("release", "download", TAG, "--repo", r.EDITOR, "--dir", directory)
        downloaded = Path(directory)
        r.verify_assets(downloaded, pair)
        r.require(all((downloaded / name).read_bytes() == (root / name).read_bytes() for name in r.ASSETS),
                  "Uploaded asset bytes differ")
    gate(pair)
    final = r.api(f"repos/{r.EDITOR}/releases/{DRAFT}")
    validate_draft(final)
    fingerprint = lambda rows: sorted((a["id"], a["name"], a["size"], a.get("digest")) for a in rows)
    r.require(fingerprint(final["assets"]) == fingerprint(assets), "Asset identity changed")
    r.api(f"repos/{r.EDITOR}/releases/{DRAFT}", "PATCH", {"draft": False, "make_latest": "true"})
    published = r.api(f"repos/{r.EDITOR}/releases/{DRAFT}")
    r.require(published.get("draft") is False and published.get("tag_name") == TAG
              and published.get("target_commitish") == SOURCE, "Cannot confirm publication")
    print(published["html_url"])


if __name__ == "__main__":
    recover(Path("release-assets"))
