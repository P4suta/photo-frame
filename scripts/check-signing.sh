#!/usr/bin/env bash
# check-signing.sh — enforcement helpers for git commit / push signing.
#
# Modes:
#   config     fail unless `commit.gpgsign` is `true` and `user.signingkey`
#              is set (pre-commit guard)
#   pre-push   read the git pre-push stdin protocol, then refuse if any
#              commit being pushed lacks a good (`G`) signature
#
# Hooked up via lefthook.yml. Lefthook is the only piece of automation
# enforcing this — a contributor running `git commit` directly without
# lefthook installed (e.g. CI surfaces) is the only loophole, and we
# accept that because CI walks the same %G? check independently.

set -Eeuo pipefail

mode="${1:-}"

case "$mode" in
  config)
    sign="$(git config --get commit.gpgsign 2>/dev/null || echo false)"
    key="$(git config --get user.signingkey 2>/dev/null || echo)"
    if [[ "$sign" != "true" ]] || [[ -z "$key" ]]; then
      echo "::error:: commit signing not configured" >&2
      echo "  commit.gpgsign = $sign" >&2
      echo "  user.signingkey = ${key:-(unset)}" >&2
      echo >&2
      echo "Project policy is that every commit must be signed. Set up signing:" >&2
      echo "  # SSH-based (recommended, GitHub-native since 2022):" >&2
      echo "  git config --global commit.gpgsign true" >&2
      echo "  git config --global tag.gpgsign true" >&2
      echo "  git config --global gpg.format ssh" >&2
      echo "  git config --global user.signingkey ~/.ssh/id_ed25519.pub" >&2
      exit 1
    fi
    ;;

  pre-push)
    # git's pre-push hook protocol: one line per ref being pushed
    #   <local ref> <local sha> <remote ref> <remote sha>
    # The zero-SHA sentinel means "new ref / nothing on the remote yet".
    zero=0000000000000000000000000000000000000000
    bad=0
    while read -r local_ref local_sha remote_ref remote_sha; do
      [[ -z "$local_sha" ]] && continue
      if [[ "$remote_sha" == "$zero" ]]; then
        # New ref: walk every commit reachable from local_sha that isn't
        # already on any other ref. Approximation: walk all reachable.
        # (A first-time push of a giant history is rare; this is fine.)
        range="$local_sha"
      else
        range="${remote_sha}..${local_sha}"
      fi
      unsigned="$(git log --pretty='%H %G?' "$range" 2>/dev/null \
        | awk '$2 != "G" {print $1}' || true)"
      if [[ -n "$unsigned" ]]; then
        echo "::error:: unsigned commit(s) cannot be pushed ($local_ref):" >&2
        while IFS= read -r sha; do
          [[ -z "$sha" ]] && continue
          subject="$(git log -1 --pretty='%h %s' "$sha")"
          echo "  $subject" >&2
        done <<<"$unsigned"
        bad=1
      fi
    done

    if [[ "$bad" -eq 1 ]]; then
      echo >&2
      echo "Sign these commits and retry. Quick fix for the tip commit:" >&2
      echo "  git commit --amend -S --no-edit" >&2
      echo "or rewrite a range:" >&2
      echo "  git rebase --root --exec 'git commit --amend --no-edit -S'" >&2
      exit 1
    fi
    ;;

  *)
    echo "Usage: $0 {config|pre-push}" >&2
    exit 2
    ;;
esac
