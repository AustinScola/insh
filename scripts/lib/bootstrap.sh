# Return if bootstrapping should be performed.
function bootstrap() {
    local default="yes"
    case "${CI:-}" in
        true | True | TRUE | yes | 1) default="no" ;;
    esac

    case "${BOOTSTRAP:-${default}}" in
        true | yes | 1) echo "yes" ;;
        false | no | 0) echo "no" ;;
        *)
            echo "Error: Unknown bootstrap value: ${BOOTSTRAP}" >&2
            exit 1
            ;;
    esac
}
