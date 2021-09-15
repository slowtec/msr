#!/usr/bin/env bash
set -euo pipefail

# Change into directory where this shell script is located
SCRIPT_ROOT=$(cd -P -- "$(dirname -- "$0")" && pwd -P)
cd "${SCRIPT_ROOT}"

# Optional: Update pre-commit scripts
pip install -U pre-commit

# Optional: Update all pre-commit hooks
pre-commit autoupdate

reset

pre-commit run --all-files
