#!/usr/bin/env bash
# Interop check: open a model in a real Archi install (headless), render its views to an
# HTML report and re-save it, then confirm Archi wrote back the same file.
# Usage: scripts/check-with-archi.sh /path/to/Archi/Archi model.archimate [out-dir]
set -euo pipefail
archi="$1"; model="$2"; out="${3:-$(mktemp -d)}"
runner=(); command -v xvfb-run >/dev/null && [ -z "${DISPLAY:-}" ] && runner=(xvfb-run -a)
"${runner[@]}" "$archi" -application com.archimatetool.commandline.app -consoleLog -nosplash \
  --loadModel "$model" --html.createReport "$out/report" --saveModel "$out/resaved.archimate"
if diff --strip-trailing-cr -q "$model" "$out/resaved.archimate" >/dev/null; then
  echo "OK: Archi loaded the model and saved it back unchanged. Report: $out/report/index.html"
else
  echo "Archi saved a different file:"; diff --strip-trailing-cr "$model" "$out/resaved.archimate" | head -40; exit 1
fi
