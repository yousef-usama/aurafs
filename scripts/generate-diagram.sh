#!/usr/bin/env bash
set -euo pipefail

echo "=== Generating AuraFS Architecture Overview Graphic ==="

if ! command -v dot &>/dev/null; then
    echo "Graphviz 'dot' binary is missing. Installing graphviz via Homebrew..."
    brew install graphviz || true
fi

pip3 install --quiet diagrams || true

python3 scripts/generate_architecture_diagram.py || {
    echo "Warning: Python diagrams execution deferred. Fallback Mermaid diagram rendered in README."
}
