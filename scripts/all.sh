#!/bin/bash

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HERE="${REPO_ROOT}/scripts"

cd "${HERE}"

./format.sh

./check.sh

./lint.sh
