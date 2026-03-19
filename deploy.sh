#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

log() {
  printf '\033[1;34m[deploy]\033[0m %s\n' "$*"
}

warn() {
  printf '\033[1;33m[warn]\033[0m %s\n' "$*"
}

err() {
  printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    err "missing command: $1"
    exit 1
  }
}

usage() {
  cat <<'EOF'
Usage:
  ./deploy.sh doctor
  ./deploy.sh build [--debug]
  ./deploy.sh relay [--listen ADDR] [--secret SECRET]
  ./deploy.sh node --relay ADDR [--namespace NAME] [--provider] [--port PORT]
  ./deploy.sh app-dev
  ./deploy.sh app-build
  ./deploy.sh help

Examples:
  ./deploy.sh doctor
  ./deploy.sh build
  ./deploy.sh relay --secret "my-secret"
  ./deploy.sh node --relay "/ip4/127.0.0.1/udp/4001/quic-v1/p2p/<PEER_ID>" --provider
  ./deploy.sh app-dev
EOF
}

doctor() {
  log "checking required tools"
  require_cmd git
  require_cmd cargo
  require_cmd rustc

  log "git:   $(git --version)"
  log "cargo: $(cargo --version)"
  log "rustc: $(rustc --version)"

  if command -v trunk >/dev/null 2>&1; then
    log "trunk: $(trunk --version)"
  else
    warn "trunk not found (required for nexlink-app)"
  fi

  if command -v node >/dev/null 2>&1; then
    log "node:  $(node --version)"
  else
    warn "node not found (required for nexlink-app)"
  fi

  if command -v npm >/dev/null 2>&1; then
    log "npm:   $(npm --version)"
  else
    warn "npm not found (required for nexlink-app)"
  fi

  if cargo tauri --version >/dev/null 2>&1; then
    log "tauri-cli: $(cargo tauri --version)"
  else
    warn "cargo tauri not found (required for desktop app build/dev)"
  fi

  log "workspace OK"
}

build_bins() {
  require_cmd cargo
  local profile="--release"
  if [[ "${1:-}" == "--debug" ]]; then
    profile=""
  fi

  log "building nexlink-relay and nexlink-node ${profile:-(--debug)}"
  cargo build $profile -p nexlink-relay -p nexlink-node

  local target_dir="target/release"
  if [[ -z "$profile" ]]; then
    target_dir="target/debug"
  fi

  log "artifacts:"
  log "- $ROOT_DIR/$target_dir/nexlink-relay"
  log "- $ROOT_DIR/$target_dir/nexlink-node"
}

run_relay() {
  require_cmd cargo

  local listen="/ip4/0.0.0.0/udp/4001/quic-v1"
  local secret="${NEXLINK_CREDENTIALS_SECRET:-}"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --listen)
        listen="$2"
        shift 2
        ;;
      --secret)
        secret="$2"
        shift 2
        ;;
      *)
        err "unknown relay arg: $1"
        usage
        exit 1
        ;;
    esac
  done

  if [[ -z "$secret" ]]; then
    err "missing credentials secret. Use --secret or export NEXLINK_CREDENTIALS_SECRET"
    exit 1
  fi

  log "starting relay on $listen"
  cargo run --release -p nexlink-relay -- \
    --listen "$listen" \
    --credentials-secret "$secret"
}

run_node() {
  require_cmd cargo

  local relay=""
  local namespace="nexlink-public"
  local provider="false"
  local port="7890"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --relay)
        relay="$2"
        shift 2
        ;;
      --namespace)
        namespace="$2"
        shift 2
        ;;
      --provider)
        provider="true"
        shift 1
        ;;
      --port)
        port="$2"
        shift 2
        ;;
      *)
        err "unknown node arg: $1"
        usage
        exit 1
        ;;
    esac
  done

  if [[ -z "$relay" ]]; then
    err "missing --relay address"
    exit 1
  fi

  local args=(
    --relay "$relay"
    --namespace "$namespace"
    --unified-port "$port"
  )

  if [[ "$provider" == "true" ]]; then
    args+=(--provider)
  fi

  log "starting node"
  cargo run --release -p nexlink-node -- "${args[@]}"
}

app_dev() {
  require_cmd cargo
  require_cmd npm
  require_cmd trunk

  log "installing frontend dependencies"
  (cd nexlink-app && npm install)

  log "building tailwind css"
  (cd nexlink-app && npm run build:tailwind)

  log "starting tauri dev"
  (cd nexlink-app/src-tauri && cargo tauri dev)
}

app_build() {
  require_cmd cargo
  require_cmd npm
  require_cmd trunk

  log "installing frontend dependencies"
  (cd nexlink-app && npm install)

  log "building tailwind css"
  (cd nexlink-app && npm run build:tailwind)

  log "building tauri app"
  (cd nexlink-app/src-tauri && cargo tauri build)
}

cmd="${1:-help}"
shift || true

case "$cmd" in
  doctor)
    doctor "$@"
    ;;
  build)
    build_bins "$@"
    ;;
  relay)
    run_relay "$@"
    ;;
  node)
    run_node "$@"
    ;;
  app-dev)
    app_dev "$@"
    ;;
  app-build)
    app_build "$@"
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    err "unknown command: $cmd"
    usage
    exit 1
    ;;
esac
