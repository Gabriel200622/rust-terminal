# Run by a POSIX shell on the SSH host, after authentication. $1 is an optional
# remote directory. Keep it separate from the shell program and fail visibly
# if it disappeared instead of silently opening a different directory.
if [ -n "$1" ]; then
    CDPATH= cd -- "$1" || exit
fi

case "${SHELL##*/}" in
zsh)
    # A private, temporary startup directory adds hooks after the user's config.
    # No dotfiles are changed. Preserve their ZDOTDIR and normal login ordering.
    _pace_dir=$(umask 077; mktemp -d "${TMPDIR:-/tmp}/pace-zsh.XXXXXXXXXX") || exec "$SHELL" -il
    _pace_user_zdotdir=${ZDOTDIR-$HOME}
    _pace_user_zdotdir_set=${ZDOTDIR+x}
    _pace_start_cwd=$1
    export _pace_dir _pace_user_zdotdir _pace_user_zdotdir_set _pace_start_cwd
    cat > "$_pace_dir/.zshenv" <<'PACE_ZSHENV'
if [[ -n $_pace_user_zdotdir_set ]]; then
    ZDOTDIR=$_pace_user_zdotdir
else
    unset ZDOTDIR
fi
[[ -r ${ZDOTDIR-$HOME}/.zshenv ]] && source "${ZDOTDIR-$HOME}/.zshenv"
_pace_user_zdotdir=${ZDOTDIR-$HOME}
_pace_user_zdotdir_set=${ZDOTDIR+x}
ZDOTDIR=$_pace_dir
PACE_ZSHENV
    cat > "$_pace_dir/.zprofile" <<'PACE_ZPROFILE'
if [[ -n $_pace_user_zdotdir_set ]]; then
    ZDOTDIR=$_pace_user_zdotdir
else
    unset ZDOTDIR
fi
[[ -r ${ZDOTDIR-$HOME}/.zprofile ]] && source "${ZDOTDIR-$HOME}/.zprofile"
_pace_user_zdotdir=${ZDOTDIR-$HOME}
_pace_user_zdotdir_set=${ZDOTDIR+x}
ZDOTDIR=$_pace_dir
PACE_ZPROFILE
    cat > "$_pace_dir/.zshrc" <<'PACE_ZSHRC'
if [[ -n $_pace_user_zdotdir_set ]]; then
    ZDOTDIR=$_pace_user_zdotdir
else
    unset ZDOTDIR
fi
[[ -r ${ZDOTDIR-$HOME}/.zshrc ]] && source "${ZDOTDIR-$HOME}/.zshrc"
_pace_user_zdotdir=${ZDOTDIR-$HOME}
_pace_user_zdotdir_set=${ZDOTDIR+x}
ZDOTDIR=$_pace_dir
PACE_ZSHRC
    cat > "$_pace_dir/.zlogin" <<'PACE_ZLOGIN'
if [[ -n $_pace_user_zdotdir_set ]]; then
    ZDOTDIR=$_pace_user_zdotdir
else
    unset ZDOTDIR
fi
command rm -f -- "$_pace_dir/.zshenv" "$_pace_dir/.zprofile" "$_pace_dir/.zshrc" "$_pace_dir/.zlogin"
# Global compinit can create its cache before the user's ZDOTDIR is restored.
command rm -f -- "$_pace_dir"/.zcompdump*(N)
command rmdir -- "$_pace_dir"
unset _pace_dir _pace_user_zdotdir _pace_user_zdotdir_set
[[ -r ${ZDOTDIR-$HOME}/.zlogin ]] && source "${ZDOTDIR-$HOME}/.zlogin"
# Login configuration may itself cd. Apply the requested split directory last.
if [[ -n $_pace_start_cwd ]]; then
    builtin cd -- "$_pace_start_cwd" || exit
fi
unset _pace_start_cwd

_pace_report_cwd() {
    # Encode bytes so Unicode, percent signs and control characters round trip.
    local LC_ALL=C ch encoded= hex
    for ch in "${(@s::)PWD}"; do
        case "$ch" in
            [a-zA-Z0-9/._~-]) encoded+=$ch ;;
            *) printf -v hex '%%%02X' "'$ch"; encoded+=$hex ;;
        esac
    done
    printf '\033]7;file://pace%s\007' "$encoded"
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd _pace_report_cwd
add-zsh-hook chpwd _pace_report_cwd
PACE_ZLOGIN
    ZDOTDIR=$_pace_dir
    export ZDOTDIR
    exec "$SHELL" -il
    ;;
*)
    # Other shells can report OSC 7 using their existing integration.
    exec "${SHELL:-/bin/sh}" -il
    ;;
esac
