#!/bin/bash

if [[ -z "${VERSION_TAG:+x}" ]]; then
    echo "Please set up VERSION_TAG variable"
    exit 2
fi

if [[ -z "${REGISTRY:+x}" ]]; then
  echo "Please set up REGISTRY variable"
  exit 2
fi

if [[ -z "${USER:+x}" ]]; then
  echo "Please set up USER variable"
  exit 2
fi

# Publish flag
if [[ "$#" -eq 1 ]] && [[ "$1" == "-p" ]]; then
  SHOULD_PUBLISH=1
fi
# Manifest name
MANIFEST_NAME="ipbot-multiarch"
# Build specific variables
SCRIPT_PATH="$(dirname -- $(readlink -f -- "$0"))"
BUILD_PATH="$(dirname -- ${SCRIPT_PATH})"
REGISTRY="$REGISTRY"
USER="$USER"
IMAGE_NAME="ipbot"
IMAGE_TAG="${VERSION_TAG}"

# Base image name
BASE_IMAGE_NAME="${REGISTRY}/${USER}/${IMAGE_NAME}:${IMAGE_TAG}"
# Create a multi-architecture manifest
buildah manifest create ${MANIFEST_NAME}

for arch in amd64 arm64; do
    ARCH_FLAGS=("--arch" "${arch}")
    if [[ "$arch" = "arm64" ]]; then
      ARCH_FLAGS=("${ARCH_FLAGS[@]}" "--variant" "v8")
    fi

    buildah bud \
      -t "$BASE_IMAGE_NAME-$arch" \
      --manifest "${MANIFEST_NAME}" \
      "${ARCH_FLAGS[@]}" "${BUILD_PATH}"
done
# Publish images to the registry
if [[ "$SHOULD_PUBLISH" -eq 1 ]]; then
  buildah manifest push --all "${MANIFEST_NAME}" \
    "docker://$BASE_IMAGE_NAME"
fi
