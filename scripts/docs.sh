#!/bin/bash

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

cd "${REPO_ROOT}"

cargo doc --offline --open