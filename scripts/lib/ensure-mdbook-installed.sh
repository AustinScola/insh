# Ensure that mdbook is installed.
function ensure_mdbook_installed() {
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

    local mdbook_version
    mdbook_version="$(yq '.mdbook.version' < "${truth_file}")"

    if [[ $(check_cargo_crate_installed mdbook "${mdbook_version}") == "no" ]]; then
        # Determine if mdbook should be installed.
        echo "Mdbook v${mdbook_version} is not installed."
        echo -n "Would you like to install it and continue? "
        if [[ "$(ask_yes_or_no)" == "no" ]]; then
            echo "Aborting..." >&2
            exit 1
        fi

        # Install mdbook.
        cargo install mdbook --locked --version "${mdbook_version}"
    fi
}
