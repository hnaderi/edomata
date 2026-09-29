#!/usr/bin/env bash
# Decide which artifacts a push to main changes, from the files it touched.
#
# Usage: detect-changes.sh <before-sha> <after-sha>
# Prints two lines, "scala=true|false" and "rust=true|false".
#
# A push counts as a Scala change when it touches the sources of a published
# Scala module (modules/*/src/main, excluding the test-only modules
# backend-tests and e2e) or the build (build.sbt, project/).
# A push counts as a Rust change when it touches a published crate (src/,
# Cargo.toml or README.md under rust/crates, excluding the test-only crates
# edomata-backend-tests and edomata-e2e) or the workspace rust/Cargo.toml.
# Tests, docs, examples, the book, website and CI files never count.
set -euo pipefail

before=${1:-}
after=${2:?usage: detect-changes.sh <before-sha> <after-sha>}

# A new branch or an unknown "before" (all zeros, force push): compare with the
# parent commit instead, or with the empty tree for a root commit.
if [ -z "$before" ] || [ "$before" = "0000000000000000000000000000000000000000" ] ||
  ! git cat-file -e "${before}^{commit}" 2>/dev/null; then
  before=$(git rev-parse --verify --quiet "${after}^" || git hash-object -t tree /dev/null)
fi

files=$(git diff --name-only "$before" "$after")

# Count the matching files instead of using "grep -q": with pipefail, grep -q
# stops at its first match, the upstream grep can then die of SIGPIPE on a huge
# diff, and the check would wrongly report false (a missed release).
# "|| true" keeps a count of zero from failing the pipeline.
count() { printf '%s\n' "$files" | grep -E "$1" | grep -Ev "$2" | grep -c . || true; }

scala=false
if [ "$(count '^(build\.sbt$|project/|modules/.+/src/main/)' '^modules/(backend-tests|e2e)/')" -gt 0 ]; then
  scala=true
fi

rust=false
if [ "$(count '^rust/(Cargo\.toml$|crates/[^/]+/(src/|Cargo\.toml$|README\.md$))' '^rust/crates/(edomata-backend-tests|edomata-e2e)/')" -gt 0 ]; then
  rust=true
fi

echo "scala=$scala"
echo "rust=$rust"
