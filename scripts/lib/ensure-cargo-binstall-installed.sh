# Ensure that cargo-binstall is installed.
function ensure_cargo_binstall_installed() {
    local HERE
    local REPO_ROOT

    local this_dir
    this_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    local repo_root
    repo_root="$(cd "${this_dir}/../.." && pwd)"

    local truth_file="${repo_root}/truth.yaml"

    local bash_lib="${repo_root}/bash-lib"
    source "${bash_lib}/ask_yes_or_no.sh"
    source "${bash_lib}/check_cargo_crate_installed.sh"
    source "${bash_lib}/ensure_brew_formula_installed.sh"

    ensure_brew_formula_installed yq

    local cargo_binstall_version
    cargo_binstall_version="$(yq '.cargo-binstall.version' < "${truth_file}")"

    if [[ $(check_cargo_crate_installed cargo-binstall "${cargo_binstall_version}") == "no" ]]; then
        # Determine if cargo-binstall should be installed.
        echo "Cargo-binstall v${cargo_binstall_version} is not installed."
        echo -n "Would you like to install it and continue? "
        if [[ "$(ask_yes_or_no)" == "no" ]]; then
            echo "Aborting..." >&2
            exit 1
        fi

        # Install cargo-binstall.
        local install_output
        if ! install_output="$(
            curl -L --proto '=https' --tlsv1.2 -sSf \
                "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/v${cargo_binstall_version}/install-from-binstall-release.sh" \
                | BINSTALL_VERSION="${cargo_binstall_version}" bash 2>&1
        )"; then
            echo "${install_output}" >&2
            echo "Failed to install cargo-binstall." >&2
            exit 1
        fi
        echo "Installed cargo-binstall v${cargo_binstall_version}."
    fi
}
