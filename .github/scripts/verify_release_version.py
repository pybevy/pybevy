"""Fail a release whose tag or prerelease flag disagrees with the package version."""

import os
import sys
import tomllib
from pathlib import Path

from packaging.version import InvalidVersion, Version


def main() -> int:
    if os.environ.get("EVENT") != "release":
        print("not a release event, nothing to verify")
        return 0

    tag = os.environ["TAG"]
    marked_prerelease = os.environ["PRERELEASE"] == "true"
    declared = tomllib.loads(Path("pyproject.toml").read_text())["project"]["version"]

    try:
        tag_version = Version(tag.removeprefix("v"))
    except InvalidVersion:
        print(f"::error::tag {tag!r} is not a PEP 440 version")
        return 1

    package_version = Version(declared)
    failures = []

    if tag_version != package_version:
        failures.append(
            f"tag {tag!r} is version {tag_version}, but pyproject.toml declares {package_version}"
        )
    if marked_prerelease != package_version.is_prerelease:
        marked = "a prerelease" if marked_prerelease else "a final release"
        actual = "a prerelease" if package_version.is_prerelease else "a final release"
        failures.append(
            f"the GitHub release is marked {marked}, but {package_version} is {actual}"
        )

    for failure in failures:
        print(f"::error::{failure}")
    if failures:
        print(f"::error::refusing to publish {package_version} to PyPI")
        return 1

    print(f"ok: tag {tag} publishes {package_version} (prerelease={package_version.is_prerelease})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
