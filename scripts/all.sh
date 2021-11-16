#!/bin/bash

set -euo pipefail

HERE="$(dirname "$(readlink -f "$BASH_SOURCE")")"

cd "${HERE}"

./format.sh

./check.sh

./lint.sh
