#!/usr/bin/env bash
# Verify Dockerfile build. Run from the project root.
# Usage: bash "<skill-dir>/scripts/verify-build.sh" [--help] [IMAGE_NAME]
# <skill-dir> is the directory that contains this skill's SKILL.md.
set -euo pipefail

usage() {
    echo "Usage: bash \"<skill-dir>/scripts/verify-build.sh\" [--help] [IMAGE_NAME]"
    echo "Run from the project root; <skill-dir> is the directory that contains this skill's SKILL.md."
    echo "Builds the Dockerfile, then reports image size and configured user."
    echo "DOCKERFILE selects the file; the current directory remains the build context."
}

if [[ "${1:-}" == "--help" && $# == 1 ]]; then
    usage
    exit 0
fi

if (( $# > 1 )); then
    usage >&2
    exit 2
fi

IMAGE="${1:-verify-build-test}"
DOCKERFILE="${DOCKERFILE:-Dockerfile}"

echo "Building image..."
docker build -f "$DOCKERFILE" -t "$IMAGE" .

echo ""
echo "Image size:"
docker images "$IMAGE" --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

echo ""
echo "User:"
docker inspect "$IMAGE" --format '{{.Config.User}}'
