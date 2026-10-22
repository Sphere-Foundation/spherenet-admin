#!/usr/bin/env bash
# Bump a spherenet-admin version pin (`KEY: '1.2.3'` or `KEY = "1.2.3"`) in FILE if it is older than LATEST.
# Usage: bump-admin-pin.sh <file> <key> <latest>   Prints `current=<x>` and `changed=true|false` (GITHUB_OUTPUT format).
set -euo pipefail

readonly SEMVER_RE='[0-9]+\.[0-9]+\.[0-9]+'

main() {
  local file="${1:-}" key="${2:-}" latest="${3:-}"
  [[ -f "$file" && -n "$key" ]] || { err "usage: $0 <file> <key> <latest>"; exit 1; }
  [[ "$latest" =~ ^${SEMVER_RE}$ ]] || { err "latest '$latest' is not X.Y.Z"; exit 1; }

  local pattern current
  pattern="$(pin_pattern "$key")"
  current="$(read_pin "$file" "$pattern")" || exit 1
  echo "current=$current"

  if ! is_outdated "$current" "$latest"; then
    echo "changed=false"
    return 0
  fi
  write_pin "$file" "$pattern" "$latest"
  echo "changed=true"
}

# Query: ERE with groups 1=boundary, 2=key..opening quote, 3=version, 4=closing quote.
# The boundary group stops KEY from matching inside a longer name (FOO_KEY).
pin_pattern() {
  local key="$1" quote="['\"]"
  printf '(^|[^[:alnum:]_])(%s[[:space:]]*[:=][[:space:]]*%s)(%s)(%s)' \
    "$key" "$quote" "$SEMVER_RE" "$quote"
}

# Query: the pinned version. Fails unless exactly one line matches.
read_pin() {
  local file="$1" pattern="$2" matches
  matches="$(grep -cE "$pattern" "$file" || true)"
  [[ "$matches" == "1" ]] || { err "$file: expected 1 pin line, found $matches"; return 1; }
  grep -E "$pattern" "$file" | sed -E "s/.*${pattern}.*/\\3/"
}

# Query: true when current < latest (semver order). Equal or newer → false.
is_outdated() {
  local current="$1" latest="$2"
  [[ "$current" != "$latest" ]] || return 1
  [[ "$(printf '%s\n%s\n' "$current" "$latest" | sort -V | tail -n1)" == "$latest" ]]
}

# Command: rewrite the version in place, keeping key, spacing and quote style.
write_pin() {
  local file="$1" pattern="$2" version="$3"
  sed -E -i "s/${pattern}/\\1\\2${version}\\4/" "$file" || { err "$file: rewrite failed"; exit 1; }
}

err() { printf 'error: %s\n' "$*" >&2; }

main "$@"
