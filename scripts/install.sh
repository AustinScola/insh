#!/bin/bash

set -euo pipefail

HERE="$(dirname "$(readlink -f "$BASH_SOURCE")")"
REPO_ROOT="$(realpath "${HERE}/..")"

cd "${REPO_ROOT}"

./scripts/release.sh

sudo cp target/release/insh /usr/local/bin/.
