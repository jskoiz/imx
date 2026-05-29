//! Hand-written shell completion scripts for the `imx` CLI.
//!
//! The `imx` binary parses arguments manually rather than through a
//! derive-based parser, so these scripts are authored explicitly and kept in
//! sync with the dispatch table in `main.rs`. Each script is a static string
//! constant, so emitting it is fully deterministic: identical bytes every run.

/// Bash completion script for `imx`.
pub const BASH: &str = r#"# bash completion for imx
# install: source this file, or place it in your bash completion directory
#   imx completions bash > /usr/local/etc/bash_completion.d/imx
_imx() {
    local cur prev words cword
    _init_completion 2>/dev/null || {
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD-1]}"
    }

    local subcommands="identify report resize resize-fit crop rotate flip flop pipeline batch-convert self-test completions"
    local global_flags="--help -h --version -V --no-auto-orient"

    # Complete the subcommand in the first position.
    if [ "${COMP_CWORD}" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "${subcommands} ${global_flags}" -- "${cur}") )
        return 0
    fi

    local cmd="${COMP_WORDS[1]}"
    case "${cmd}" in
        completions)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "${cur}") )
            return 0
            ;;
        pipeline)
            if [[ "${cur}" == --* ]]; then
                COMPREPLY=( $(compgen -W "--op" -- "${cur}") )
                return 0
            fi
            _filedir
            return 0
            ;;
        batch-convert)
            case "${prev}" in
                --to)
                    COMPREPLY=( $(compgen -W "BMP FARBFELD JPEG QOI PBM PGM PNG PPM WEBP" -- "${cur}") )
                    return 0
                    ;;
                --output-dir)
                    _filedir -d
                    return 0
                    ;;
                --resize|--resize-fit|--quality)
                    return 0
                    ;;
            esac
            if [[ "${cur}" == --* ]]; then
                COMPREPLY=( $(compgen -W "--to --output-dir --resize --resize-fit --quality" -- "${cur}") )
                return 0
            fi
            _filedir
            return 0
            ;;
        identify|report)
            if [[ "${cur}" == --* ]]; then
                COMPREPLY=( $(compgen -W "--json" -- "${cur}") )
                return 0
            fi
            _filedir
            return 0
            ;;
        self-test)
            return 0
            ;;
        *)
            _filedir
            return 0
            ;;
    esac
}
complete -F _imx imx
"#;

/// Zsh completion script for `imx`.
pub const ZSH: &str = r#"#compdef imx
# zsh completion for imx
# install: place this file as _imx on your $fpath, e.g.
#   imx completions zsh > "${fpath[1]}/_imx"

_imx() {
    local context state state_descr line
    typeset -A opt_args

    local -a subcommands
    subcommands=(
        'identify:Print stable image metadata'
        'report:Print JSON support report for a single input'
        'resize:Resize to an exact, single-axis, or percent geometry'
        'resize-fit:Resize preserving aspect ratio to fit a box'
        'crop:Crop a bounded region'
        'rotate:Rotate clockwise by 90, 180, or 270 degrees'
        'flip:Flip vertically'
        'flop:Flop horizontally'
        'pipeline:Apply ordered ops in one decode/encode pass'
        'batch-convert:Convert many inputs into an output directory'
        'self-test:Run the offline install confidence check'
        'completions:Print a shell completion script'
    )

    _arguments -C \
        '(- *)'{-h,--help}'[Show help]' \
        '(- *)'{-V,--version}'[Show version]' \
        '--no-auto-orient[Disable EXIF/TIFF orientation auto-rotation]' \
        '1: :->command' \
        '*:: :->args' \
        && return 0

    case "${state}" in
        command)
            _describe -t commands 'imx command' subcommands
            ;;
        args)
            case "${line[1]}" in
                completions)
                    _values 'shell' bash zsh fish
                    ;;
                identify|report)
                    _arguments \
                        '--json[Emit JSON output]' \
                        '*:input file:_files'
                    ;;
                batch-convert)
                    _arguments \
                        '--to[Output format]:format:(BMP FARBFELD JPEG QOI PBM PGM PNG PPM WEBP)' \
                        '--output-dir[Output directory]:directory:_files -/' \
                        '--resize[Resize geometry]:geometry:' \
                        '--resize-fit[Fit geometry]:geometry:' \
                        '--quality[JPEG quality 1..=100]:quality:' \
                        '*:input file:_files'
                    ;;
                pipeline)
                    _arguments \
                        '*--op[Operation to apply, left-to-right]:op:' \
                        '*:file:_files'
                    ;;
                self-test)
                    ;;
                *)
                    _arguments '*:file:_files'
                    ;;
            esac
            ;;
    esac
}

_imx "$@"
"#;

/// Fish completion script for `imx`.
pub const FISH: &str = r#"# fish completion for imx
# install: imx completions fish > ~/.config/fish/completions/imx.fish

function __fish_imx_no_subcommand
    set -l cmd (commandline -opc)
    if test (count $cmd) -eq 1
        return 0
    end
    return 1
end

# Top-level flags.
complete -c imx -n '__fish_imx_no_subcommand' -s h -l help -d 'Show help'
complete -c imx -n '__fish_imx_no_subcommand' -s V -l version -d 'Show version'
complete -c imx -n '__fish_imx_no_subcommand' -l no-auto-orient -d 'Disable EXIF/TIFF orientation auto-rotation'

# Subcommands.
complete -c imx -f -n '__fish_imx_no_subcommand' -a identify -d 'Print stable image metadata'
complete -c imx -f -n '__fish_imx_no_subcommand' -a report -d 'Print JSON support report'
complete -c imx -f -n '__fish_imx_no_subcommand' -a resize -d 'Resize to a geometry'
complete -c imx -f -n '__fish_imx_no_subcommand' -a resize-fit -d 'Resize preserving aspect ratio'
complete -c imx -f -n '__fish_imx_no_subcommand' -a crop -d 'Crop a bounded region'
complete -c imx -f -n '__fish_imx_no_subcommand' -a rotate -d 'Rotate clockwise 90/180/270'
complete -c imx -f -n '__fish_imx_no_subcommand' -a flip -d 'Flip vertically'
complete -c imx -f -n '__fish_imx_no_subcommand' -a flop -d 'Flop horizontally'
complete -c imx -f -n '__fish_imx_no_subcommand' -a pipeline -d 'Apply ordered ops in one pass'
complete -c imx -f -n '__fish_imx_no_subcommand' -a batch-convert -d 'Convert many inputs'
complete -c imx -f -n '__fish_imx_no_subcommand' -a self-test -d 'Run the install confidence check'
complete -c imx -f -n '__fish_imx_no_subcommand' -a completions -d 'Print a shell completion script'

# completions <shell>
complete -c imx -f -n '__fish_seen_subcommand_from completions' -a 'bash zsh fish' -d 'Shell'

# identify / report flags.
complete -c imx -n '__fish_seen_subcommand_from identify report' -l json -d 'Emit JSON output'

# pipeline flags.
complete -c imx -n '__fish_seen_subcommand_from pipeline' -l op -d 'Operation to apply, left-to-right' -x

# batch-convert flags.
complete -c imx -n '__fish_seen_subcommand_from batch-convert' -l to -d 'Output format' -x -a 'BMP FARBFELD JPEG QOI PBM PGM PNG PPM WEBP'
complete -c imx -n '__fish_seen_subcommand_from batch-convert' -l output-dir -d 'Output directory' -x -a '(__fish_complete_directories)'
complete -c imx -n '__fish_seen_subcommand_from batch-convert' -l resize -d 'Resize geometry' -x
complete -c imx -n '__fish_seen_subcommand_from batch-convert' -l resize-fit -d 'Fit geometry' -x
complete -c imx -n '__fish_seen_subcommand_from batch-convert' -l quality -d 'JPEG quality 1..=100' -x
"#;
