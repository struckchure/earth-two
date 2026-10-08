#!/bin/sh
# Build the networking-only browser adapter. Emscripten ships a suitable Node
# runtime; use it when a Go-only deployment image has no recent system Node.
set -eu

if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1 || [ "$(node -p 'Number(process.versions.node.split(".")[0]) >= 18' 2>/dev/null || true)" != "true" ]; then
    stdb_node_dir=$(python3 - <<'PY'
import ast, os, pathlib
config = pathlib.Path(os.environ.get('EM_CONFIG', str(pathlib.Path.home() / '.emscripten')))
if config.exists():
    tree = ast.parse(config.read_text())
    for item in tree.body:
        if isinstance(item, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'NODE_JS' for t in item.targets):
            value = ast.literal_eval(item.value)
            node = value[0] if isinstance(value, list) else value
            print(pathlib.Path(node).parent)
            break
PY
)
    if [ -n "$stdb_node_dir" ]; then PATH="$stdb_node_dir:$PATH"; export PATH; fi
fi

if ! command -v npm >/dev/null 2>&1; then
    echo "Browser networking requires Node.js 18+ and npm (or Emscripten's bundled Node)." >&2
    exit 1
fi
cd web/spacetime
npm ci --ignore-scripts
npm run build
