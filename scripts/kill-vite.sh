#!/usr/bin/env bash
# Stop Nightdesk Vite (npm run dev / preview) and free its ports.
#
# Desde la raíz del repo:
#   ./scripts/kill-vite.sh
# Si no es ejecutable:
#   bash scripts/kill-vite.sh
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORTS=(1420 1421)
PIDS=""

trim() {
  local s="${1:-}"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

add_pid() {
  local pid="${1:-}"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 0
  [[ "$pid" == "$$" || "$pid" == "$PPID" ]] && return 0
  kill -0 "$pid" 2>/dev/null || return 0
  case " $PIDS " in
    *" $pid "*) ;;
    *) PIDS="${PIDS} ${pid}" ;;
  esac
}

add_ancestors_if_npm() {
  local pid="${1:-}"
  local parent cmd
  parent="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')"
  [[ "$parent" =~ ^[0-9]+$ ]] || return 0
  cmd="$(ps -o command= -p "$parent" 2>/dev/null || true)"
  if [[ "$cmd" == *"npm run"* ]] || [[ "$cmd" == *"npm "* && "$cmd" == *"vite"* ]]; then
    add_pid "$parent"
  fi
}

# Listeners on Vite / HMR ports.
if command -v lsof >/dev/null 2>&1; then
  for port in "${PORTS[@]}"; do
    while read -r pid; do
      add_pid "$pid"
    done < <(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)
  done
fi

# Vite / esbuild started from this repo.
while read -r pid; do
  add_pid "$pid"
done < <(pgrep -f "hotel-nightdesk/node_modules/.bin/vite" 2>/dev/null || true)

while read -r pid; do
  add_pid "$pid"
done < <(pgrep -f "hotel-nightdesk/node_modules/@esbuild/" 2>/dev/null || true)

# npm wrappers that spawned those processes.
for pid in $PIDS; do
  add_ancestors_if_npm "$pid"
done

PIDS="$(trim "$PIDS")"

if [[ -z "$PIDS" ]]; then
  echo "No había ninguna sesión de Vite de Nightdesk."
  exit 0
fi

echo "Matando Vite de Nightdesk: $PIDS"
kill $PIDS 2>/dev/null || true
sleep 0.4

STILL=""
for pid in $PIDS; do
  if kill -0 "$pid" 2>/dev/null; then
    STILL="${STILL} ${pid}"
  fi
done
STILL="$(trim "$STILL")"

if [[ -n "$STILL" ]]; then
  echo "Forzando: $STILL"
  kill -9 $STILL 2>/dev/null || true
  sleep 0.2
fi

BUSY=""
if command -v lsof >/dev/null 2>&1; then
  for port in "${PORTS[@]}"; do
    if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      BUSY="${BUSY} ${port}"
    fi
  done
fi
BUSY="$(trim "$BUSY")"

if [[ -n "$BUSY" ]]; then
  echo "Algunos puertos siguen ocupados: $BUSY"
  exit 1
fi

echo "Vite detenido. Puerto 1420 libre."
