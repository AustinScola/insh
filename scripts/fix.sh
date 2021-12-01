#!/bin/bash

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

cd "${REPO_ROOT}"

cargo clippy --fix --allow-staged --allow-dirty