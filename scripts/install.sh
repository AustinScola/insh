#!/bin/bash

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

cd "${REPO_ROOT}"

./scripts/release.sh

sudo cp target/release/insh /usr/local/bin/.
