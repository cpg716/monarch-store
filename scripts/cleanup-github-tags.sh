#!/usr/bin/env bash
# Remove old Git tags from remote and local, keeping only v0.5.0-alpha and v0.4.7-alpha.
# Run from repo root: bash scripts/cleanup-github-tags.sh
set -euo pipefail
KEEP="v0.5.0-alpha v0.4.7-alpha"
REMOTE="${REMOTE:-origin}"

to_delete=()
while read -r tag; do
  case " $KEEP " in
    *" $tag "*) ;;
    *) to_delete+=("$tag") ;;
  esac
done < <(git tag -l)

echo "Keeping: $KEEP"
echo "Deleting ${#to_delete[@]} tag(s) from $REMOTE and locally."
read -r -p "Proceed? [y/N] " ans
[[ "${ans,,}" == "y" ]] || exit 0

for tag in "${to_delete[@]}"; do
  git push "$REMOTE" --delete "$tag" 2>/dev/null || true
  git tag -d "$tag" 2>/dev/null || true
done
echo "Done."
