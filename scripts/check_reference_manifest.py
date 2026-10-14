"""Verifica el manifiesto de fixtures de referencia (B18-001).

Comprueba que `assets/fixtures/reference/manifest.json` cumple el esquema minimo
(version, clases de procedencia y entradas), que cada `path` existe, que su
SHA-256 coincide con el hash declarado y que la procedencia es una de las clases
permitidas. No requiere dependencias externas; apto para CI y Windows.

Uso:
    python scripts/check_reference_manifest.py
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "assets/fixtures/reference/manifest.json"
REFERENCE_ROOT = MANIFEST_PATH.parent

REQUIRED_ENTRY_FIELDS = ("path", "provenance", "sha256", "description", "conditions")


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    if not MANIFEST_PATH.is_file():
        print(f"ERROR: no existe {MANIFEST_PATH}")
        return 1

    try:
        manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        print(f"ERROR: JSON invalido en {MANIFEST_PATH}: {error}")
        return 1

    errors: list[str] = []

    if manifest.get("version") != 1:
        errors.append(f"version no soportada: {manifest.get('version')!r}")

    classes = manifest.get("provenance_classes")
    if not isinstance(classes, list) or not classes:
        errors.append("falta provenance_classes (lista no vacia)")
        classes = []
    allowed = set(classes)

    entries = manifest.get("entries")
    if not isinstance(entries, list) or not entries:
        errors.append("falta entries (lista no vacia)")
        entries = []

    seen_paths: set[str] = set()
    for index, entry in enumerate(entries):
        label = f"entries[{index}]"
        if not isinstance(entry, dict):
            errors.append(f"{label}: no es un objeto")
            continue
        for field in REQUIRED_ENTRY_FIELDS:
            value = entry.get(field)
            if not isinstance(value, str) or not value.strip():
                errors.append(f"{label}: campo '{field}' ausente o vacio")
        relative = entry.get("path")
        if not isinstance(relative, str) or not relative:
            continue
        if relative in seen_paths:
            errors.append(f"{label}: path duplicado '{relative}'")
        seen_paths.add(relative)

        target = REFERENCE_ROOT / relative
        if not target.is_file():
            errors.append(f"{label}: no existe '{relative}'")
            continue

        declared = entry.get("sha256", "")
        actual = sha256_of(target)
        if declared.lower() != actual:
            errors.append(
                f"{label}: sha256 no coincide para '{relative}'\n"
                f"    declarado: {declared}\n"
                f"    real:      {actual}"
            )

        provenance = entry.get("provenance")
        if provenance not in allowed:
            errors.append(
                f"{label}: procedencia '{provenance}' fuera de {sorted(allowed)}"
            )

    if errors:
        print(f"FALLO: {len(errors)} problema(s) en {MANIFEST_PATH.relative_to(REPO_ROOT)}")
        for error in errors:
            print(f"  - {error}")
        return 1

    print(
        f"OK: manifiesto valido ({len(entries)} fixtures verificados) en "
        f"{MANIFEST_PATH.relative_to(REPO_ROOT)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
