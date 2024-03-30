#! /usr/bin/env nix-shell
#! nix-shell -i bash -p rustup cargo-cross

set -e

declare -A targets
targets["linux/amd64"]="x86_64-unknown-linux-musl"
targets["linux/arm64"]="aarch64-unknown-linux-musl"

package_name=$(cat Cargo.toml | grep 'name' | awk '{print $3}' | tr -d '"')
package_version=$(cat Cargo.toml | grep 'version' | head -1 | awk '{print $3}' | tr -d '"')

echo "package_name: $package_name"
echo "package_version: $package_version"
echo "------------------------"


main() {
  check_images

  for platform in "${!targets[@]}"; do
    tag=$(echo "${platform//\//_}" | tr -d 'linux_' | xargs -I {} echo {})

    echo "Compiling for ${targets[$platform]}"
    # cargo build --release # needed to compile dynamic linked libraries
    cross build --target "${targets[$platform]}" --release

    echo "Building image for $platform with tag: $tag"
    podman_build "$tag" "$platform" "${targets[$platform]}"
  done

  podman system prune -f

  podman_manifest "v$package_version"
  podman_manifest "latest"

  podman rmi kennycallado/${package_name}:v${package_version}
  podman rmi kennycallado/${package_name}:latest

  podman system prune -f
}

check_images() {
  for platform in "${!targets[@]}"; do
    tag=$(echo "${platform//\//_}" | tr -d 'linux_' | xargs -I {} echo {})
    if podman inspect kennycallado/${package_name}:v${package_version}-${tag} &> /dev/null; then
      echo "Removing existing image kennycallado/${package_name}:v${package_version}-${tag}"
      podman rmi kennycallado/${package_name}:v${package_version}-${tag}
    fi
  done

  if podman inspect kennycallado/${package_name}:v${package_version} &> /dev/null; then
    echo "Removing existing image kennycallado/${package_name}:v${package_version}"
    podman rmi kennycallado/${package_name}:v${package_version}
  fi

  if podman inspect kennycallado/${package_name}:latest &> /dev/null; then
    echo "Removing existing image kennycallado/${package_name}:latest"
    podman rmi kennycallado/${package_name}:latest
  fi
}

podman_manifest() {
  local version=$1

  echo $version
  podman manifest create --amend kennycallado/${package_name}:${version}

  for platform in "${!targets[@]}"; do
    tag=$(echo "${platform//\//_}" | tr -d 'linux_' | xargs -I {} echo {})
    arch=${platform#*/}

    echo "------------------------"
    echo "version: $version"
    echo "platform: $platform"
    echo "arch: $arch"
    echo "------------------------"

    podman manifest add --arch "${tag}" kennycallado/"${package_name}":"${version}" kennycallado/"${package_name}":v"${package_version}"-"${tag}"
    podman manifest push kennycallado/"${package_name}":"${version}" docker://kennycallado/"${package_name}":"${version}"
  done
}

podman_build() {
  local tag=$1
  local platform=$2
  local target=$3

  podman build --no-cache --pull \
    --platform ${platform} \
    -t kennycallado/${package_name}:v${package_version}-${tag} \
    --build-arg PACKAGE_NAME=${package_name} \
    --build-arg TARGET=${target} \
    -f ./Containerfile .

  podman push kennycallado/${package_name}:v${package_version}-${tag}
  podman rmi  kennycallado/${package_name}:v${package_version}-${tag}
}

main "$@"
