# Ensure that rumdl is installed.
function ensure_rumdl_installed() {
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

    local rumdl_version
    rumdl_version="$(yq '.rumdl.version' < "${truth_file}")"

    if [[ $(check_cargo_crate_installed rumdl "${rumdl_version}") == "no" ]]; then
        # Determine if rumdl should be installed.
        echo "Rumdl v${rumdl_version} is not installed."
        echo -n "Would you like to install it and continue? "
        if [[ "$(ask_yes_or_no)" == "no" ]]; then
            echo "Aborting..." >&2
            exit 1
        fi

        # Install rumdl.
        source "${this_dir}/ensure-cargo-binstall-installed.sh"
        ensure_cargo_binstall_installed
        cargo binstall rumdl --locked --version "${rumdl_version}" --no-confirm \
            --disable-strategies compile
    fi
}
