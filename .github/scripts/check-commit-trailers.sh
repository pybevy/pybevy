#!/usr/bin/env bash
# Rejects AI assistant attribution trailers before they reach public history.
set -euo pipefail

usage() {
    echo "usage: $0 <base-sha> <head-sha>    scan every commit in a range" >&2
    echo "       $0 --message-file <path>    scan one message (commit-msg hook)" >&2
    exit 2
}

# Attribution trailers and tool URLs only. Prose that merely names an assistant
# is legitimate, and a human Co-authored-by trailer must keep working.
PATTERNS=(
    '^[[:space:]]*co-authored-by:.*(anthropic|claude|copilot|chatgpt|openai|cursor|codex|gemini)'
    '^[[:space:]]*(claude-session|assisted-by|ai-assisted-by|generated-by):'
    'generated with \[?(claude|copilot|chatgpt|codex|cursor)'
    'https?://claude\.(ai|com)/'
)

scan() {
    local text=$1 pattern hit=1 found=""
    for pattern in "${PATTERNS[@]}"; do
        if grep -Eiqs -- "$pattern" <<<"$text"; then
            found+=$(grep -Eins -- "$pattern" <<<"$text")$'\n'
            hit=0
        fi
    done
    # A line can match several patterns; report it once.
    [[ $hit -eq 0 ]] && printf '%s' "$found" | sort -t: -k1,1n -u | sed 's/^/      /'
    return $hit
}

explain() {
    cat >&2 <<'EOF'

The public repository records maintainer authorship only. Remove the trailer
and amend, then force-push the branch:

    git rebase -i <base>        # reword each flagged commit
    git push --force-with-lease

Note that a squash merge composes its body from the commit messages in the
pull request, so a trailer on any commit reaches main even when squashed.
EOF
}

if [[ ${1:-} == --message-file ]]; then
    [[ -n ${2:-} && -r ${2:-} ]] || usage
    if output=$(scan "$(cat -- "$2")"); then
        echo "error: AI attribution trailer found" >&2
        printf '%s\n' "$output" >&2
        explain
        exit 1
    fi
    echo "OK: no AI attribution trailer"
    exit 0
fi

[[ $# -eq 2 ]] || usage
base=$1
head=$2

# A first push, a force-push, or a shallow base leaves no usable range.
if [[ -z $base || $base =~ ^0+$ ]] || ! git rev-parse --verify -q "$base^{commit}" >/dev/null; then
    echo "note: base $base unusable, scanning the tip commit only" >&2
    mapfile -t commits < <(git rev-list -1 "$head")
else
    mapfile -t commits < <(git rev-list "$base..$head")
fi

failed=0
for sha in "${commits[@]}"; do
    [[ -n $sha ]] || continue
    if output=$(scan "$(git log -1 --format='%B' "$sha")"); then
        printf 'FAIL %.12s %s\n' "$sha" "$(git log -1 --format='%s' "$sha")" >&2
        printf '%s\n' "$output" >&2
        failed=1
    fi
done

if ((failed)); then
    explain
    exit 1
fi

echo "OK: no AI attribution trailers in ${#commits[@]} commit(s)"
