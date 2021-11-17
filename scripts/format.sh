#!/bin/bash

set -euo pipefail

HERE="$(dirname "$(readlink -f "$BASH_SOURCE")")"
REPO_ROOT="$(realpath "${HERE}/..")"

cd "${REPO_ROOT}"

cargo fmt "$@"
