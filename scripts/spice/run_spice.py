"""Ejecuta un netlist de ngspice en modo batch con la instalación local.

Busca el binario en este orden:
1. Variable de entorno `NGSPICE_BIN` (ruta al ejecutable).
2. `tools/ngspice/Spice64/bin/ngspice_con.exe` dentro del repositorio.
3. `ngspice_con` o `ngspice` en el PATH.

Uso:
    python scripts/spice/run_spice.py scripts/spice/b18n_tone_1967.cir
    python scripts/spice/run_spice.py netlist.cir --log target/reports/tone.log

El proceso se ejecuta con directorio de trabajo el del netlist para que las
inclusiones relativas (`.include`, `.lib`) funcionen. La salida de ngspice se
guarda en un log (por defecto `<netlist>.log`) y se muestran las líneas de
error encontradas.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LOCAL_NGSPICE = REPO_ROOT / "tools" / "ngspice" / "Spice64" / "bin"


def find_ngspice() -> Path:
    env = os.environ.get("NGSPICE_BIN")
    if env:
        candidate = Path(env)
        if candidate.is_file():
            return candidate
        raise SystemExit(f"NGSPICE_BIN no apunta a un archivo: {env}")

    for name in ("ngspice_con.exe", "ngspice_con", "ngspice"):
        candidate = LOCAL_NGSPICE / name
        if candidate.is_file():
            return candidate

    for name in ("ngspice_con", "ngspice"):
        found = shutil.which(name)
        if found:
            return Path(found)

    raise SystemExit(
        "no se encontró ngspice; instale ngspice 47 en tools/ngspice "
        "(véase scripts/spice/README.md) o defina NGSPICE_BIN"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Runner de ngspice en modo batch")
    parser.add_argument("netlist", type=Path, help="netlist .cir a ejecutar")
    parser.add_argument(
        "--log",
        type=Path,
        default=None,
        help="ruta del log (por defecto: <netlist>.log)",
    )
    args = parser.parse_args()

    netlist = args.netlist.resolve()
    if not netlist.is_file():
        raise SystemExit(f"netlist no encontrado: {netlist}")

    log = args.log or netlist.with_suffix(netlist.suffix + ".log")
    log = log.resolve()
    log.parent.mkdir(parents=True, exist_ok=True)

    binary = find_ngspice()
    completed = subprocess.run(
        [str(binary), "-b", "-o", str(log), str(netlist.name)],
        cwd=str(netlist.parent),
        check=False,
    )

    errors = []
    if log.is_file():
        for line in log.read_text(encoding="utf-8", errors="replace").splitlines():
            if "Error:" in line or "error:" in line:
                errors.append(line.strip())

    print(f"ngspice: {binary}")
    print(f"netlist: {netlist}")
    print(f"log:     {log}")
    print(f"salida:  {completed.returncode}")
    for line in errors:
        print(f"  {line}")

    if completed.returncode != 0 or errors:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
