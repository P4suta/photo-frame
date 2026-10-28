#!/usr/bin/env bash
# Verify signing configuration and introduced commits in the Git push protocol.

set -Eeuo pipefail
export GIT_NO_REPLACE_OBJECTS=1

mode="${1:-}"

case "${mode}" in
config)
	sign="$(git config --get commit.gpgsign 2>/dev/null || echo false)"
	key="$(git config --get user.signingkey 2>/dev/null || echo)"
	if [[ ${sign} != "true" ]] || [[ -z ${key} ]]; then
		echo "::error:: commit signing not configured" >&2
		echo "  commit.gpgsign = ${sign}" >&2
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
	protocol=''
	input_status=0
	protocol_file="$(mktemp)"
	trap 'rm -f -- "${protocol_file}"' EXIT
	if ! cat >"${protocol_file}"; then
		echo '::error:: cannot read the Git pre-push protocol' >&2
		exit 1
	fi
	IFS= read -r -d '' protocol <"${protocol_file}" || input_status=$?
	if [[ ${input_status} -ne 1 ]]; then
		echo '::error:: cannot read the Git pre-push protocol' >&2
		exit 1
	fi
	rm -f -- "${protocol_file}"
	trap - EXIT
	[[ -n ${protocol} ]] || exit 0

	object_format="$(git rev-parse --show-object-format)"
	case "${object_format}" in
	sha1) oid_size=40 ;;
	sha256) oid_size=64 ;;
	*)
		echo '::error:: unsupported Git object format' >&2
		exit 1
		;;
	esac
	printf -v zero '%*s' "${oid_size}" ''
	zero="${zero// /0}"
	record_pattern="^([^[:space:]]+) ([0-9a-f]{${oid_size}}) (refs/[^[:space:]]+) ([0-9a-f]{${oid_size}})$"
	destination="${3:-${2:-${PHOTO_FRAME_PUSH_DESTINATION:-}}}"
	snapshot_loaded=false
	remote_tips=()

	while [[ -n ${protocol} ]]; do
		if [[ ${protocol} != *$'\n'* ]]; then
			echo '::error:: unterminated Git pre-push record' >&2
			exit 1
		fi
		record="${protocol%%$'\n'*}"
		protocol="${protocol#*$'\n'}"
		if [[ ! ${record} =~ ${record_pattern} ]]; then
			echo '::error:: malformed Git pre-push record' >&2
			exit 1
		fi
		local_ref="${BASH_REMATCH[1]}"
		local_sha="${BASH_REMATCH[2]}"
		remote_ref="${BASH_REMATCH[3]}"
		remote_sha="${BASH_REMATCH[4]}"
		git check-ref-format "${remote_ref}"
		if [[ ${local_sha} == "${zero}" ]]; then
			[[ ${local_ref} == '(delete)' && ${remote_sha} != "${zero}" ]] || exit 1
			continue
		fi
		[[ ${local_ref} != '(delete)' ]] || exit 1

		if [[ ${remote_sha} == "${zero}" ]]; then
			if [[ ${snapshot_loaded} == false ]]; then
				[[ -n ${destination} ]] || {
					echo '::error:: missing push destination' >&2
					exit 1
				}
				advertised="$(git ls-remote --heads --refs -- "${destination}")"
				if [[ -n ${advertised} ]]; then
					while IFS=$'\t' read -r tip ref; do
						if [[ ! ${tip} =~ ^[0-9a-f]{${oid_size}}$ || ${ref} != refs/heads/* ]]; then
							echo '::error:: malformed destination ref' >&2
							exit 1
						fi
						git check-ref-format "${ref}"
						remote_tips+=("${tip}")
					done <<<"${advertised}"
					git fetch --quiet --no-tags --no-prune --no-prune-tags --no-write-fetch-head --no-recurse-submodules \
						--no-auto-maintenance --refmap= -- "${destination}" "${remote_tips[@]}"
				fi
				snapshot_loaded=true
			fi
			# Exclude only the destination heads observed and fetched for this push.
			revisions=("${local_sha}" --not "${remote_tips[@]}")
		else
			revisions=("${remote_sha}..${local_sha}")
		fi
		signatures="$(git log --no-color --no-decorate --no-notes --no-show-signature \
			--format='%H %G?' "${revisions[@]}" --)"
		if [[ -n ${signatures} ]]; then
			while read -r sha verdict extra; do
				if [[ ! ${sha} =~ ^[0-9a-f]{${oid_size}}$ || ${verdict} != G || -n ${extra} ]]; then
					echo "::error:: commit signature verification failed (${local_ref})" >&2
					exit 1
				fi
			done <<<"${signatures}"
		fi
	done
	;;

*)
	echo "Usage: $0 {config|pre-push}" >&2
	exit 2
	;;
esac
