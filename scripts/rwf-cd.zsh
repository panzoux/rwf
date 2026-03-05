#!/bin/zsh
# Wrapper script for rwf that changes directory on exit (zsh version)
# 
# Usage:
#   Source this script in your ~/.zshrc:
#     source /path/to/rwf-cd.zsh
#     alias rwf='rwf_cd'

rwf_cd() {
    # Run rwf with -cwd flag and capture the output directory
    local output_dir
    output_dir=$(rwf --cwd "$@")
    local exit_code=$?
    
    # If rwf exited successfully and output a directory, change to it
    if [[ $exit_code -eq 0 ]] && [[ -n "$output_dir" ]] && [[ -d "$output_dir" ]]; then
        cd "$output_dir" || return 1
    fi
    
    return $exit_code
}
