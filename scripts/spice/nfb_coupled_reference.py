"""B18-009 independent ngspice reference of the differential OT equivalent.

Uses the same declared continuous circuit as OutputNetwork, not Rust output.
Frozen rails 380/460/505/500/-55 V; ideal tone sources, Rmix/2 = 135k.
NFB senses an unloaded ideal 16-ohm tap, as in the existing OT one-port.
Writes AC magnitude/phase for open/closed NFB and nominal/reactive loads.
"""
import csv
import hashlib
import json
import math
from pathlib import Path
import nfb_loop_reference as reference

ROOT = reference.REPO_ROOT
OUT = ROOT / "assets/fixtures/reference/nfb"
FREQUENCIES = [93.75, 375.0, 1500.0, 6000.0, 12000.0, 18000.0]


def netlist(load, feedback):
    text = reference.netlist(load, feedback, True)
    text = text.replace("rsrc inx pin7 270k", "rsrc inx pin7 135k")
    text = text.replace("rg7 pin7 0 5.6meg", "* no untraced grid-leak resistor")
    start = text.index("rdc1 pa a 150")
    end = text.index("esec s1 0", start)
    # Separate common and differential ports. This avoids assigning the core
    # loss asymmetrically to one DC half-primary in the exploratory netlist.
    text = text[:start] + """* Common-mode half-primary DCR, 150 ohms per side.
Bcommon1 pa bpl I = {(v(pa)+v(pb)-2*v(bpl))/300}
Bcommon2 pb bpl I = {(v(pa)+v(pb)-2*v(bpl))/300}
Ed d 0 pa pb 1
Fdiff1 pa 0 Ed -1
Fdiff2 pb 0 Ed 1
Rdc d a 300
Rcore a f 500
Lm f 0 28.7
Ll f g 12m
Cp a 0 100p
""" + text[end:]
    text = text.replace("esec s1 0 b g", "esec s1 0 g 0").replace("fpri g b vsense", "fpri g 0 vsense").replace("{-1/23.18}", "{1/23.18}")
    # Precision parameters from the overlay; no independent fitting to Rust.
    calibration = json.loads((ROOT/"assets/models/b18n_1967_calibration.json").read_text(encoding="utf-8"))["assumptions"]
    for tube, subckt in [("7199.pentode", "k7199p"), ("7199.triode", "k7199t")]:
        prefix = f"tube_models.{tube}.koren."
        import re
        params = {key[len(prefix):]: entry["value"] for key, entry in calibration.items() if key.startswith(prefix)}
        declaration = re.search(rf"\.subckt {subckt} .*", text).group()
        for key, value in params.items():
            declaration = re.sub(rf"\b{key}=[^ ]+", f"{key}={value:.15g}", declaration)
        text = re.sub(rf"\.subckt {subckt} .*", declaration, text)
    start = text.index(".op\n.options")
    commands = [".options method=gear", ".control", "set numdgt=15", "op",
                f"wrdata {reference.WORK.as_posix()}/dc_{load}_{feedback}.txt v(pin7) v(pin2) v(pin3) v(pin6) v(pin9) v(pin1) v(pin8) v(g1) v(g2) v(pa) v(pb)"]
    for index, frequency in enumerate(FREQUENCIES):
        commands += [f"ac lin 1 {frequency} {frequency}", f"wrdata {reference.WORK.as_posix()}/point_{index}.txt vm(s4) vp(s4)"]
    commands += [".endc", ".end"]
    return text[:start] + "\n".join(commands) + "\n"


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rows, dc_rows = [], []
    for load in ["nominal", "reactiva"]:
        for feedback in [False, True]:
            text = netlist(load, feedback)
            log, status = reference.run(f"coupled_{load}_{feedback}", text)
            if status != "0" or "Error" in log:
                raise RuntimeError(log)
            (OUT/f"{load}_{feedback}.cir").write_text(text.replace(reference.WORK.as_posix(), "target/spice_loop"), encoding="ascii")
            for index, frequency in enumerate(FREQUENCIES):
                fields = (reference.WORK/f"point_{index}.txt").read_text().splitlines()[-1].split()
                magnitude, phase = float(fields[1]), float(fields[3])
                rows.append([load, int(feedback), frequency, 20*math.log10(magnitude), math.degrees(phase)])
            fields = (reference.WORK/f"dc_{load}_{feedback}.txt").read_text().splitlines()[-1].split()
            dc_rows.append([load, int(feedback)] + [float(v) for v in fields[1::2]])
    for name, header, data in [("coupled_ac.csv", ["load", "feedback", "frequency_hz", "gain_db", "phase_deg"], rows),
                               ("coupled_dc.csv", ["load", "feedback", "g", "p", "s", "k", "tg", "tp", "tk", "u", "l", "a", "b"], dc_rows)]:
        path = OUT/name
        with path.open("w", newline="", encoding="utf-8") as stream:
            writer = csv.writer(stream, lineterminator="\n")
            writer.writerow(header)
            writer.writerows(data)
        manifest_path = OUT.parent/"manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["entries"] = [entry for entry in manifest["entries"] if entry["path"] != f"nfb/{name}"]
        manifest["entries"].append(dict(path=f"nfb/{name}", provenance="independent-SPICE", sha256=hashlib.sha256(path.read_bytes()).hexdigest(), description="B18-009 coupled output network, frozen rails, unloaded NFB sensing tap.", conditions="ngspice 47; 380/460/505/500/-55 V; Rmix/2=135k; RCA capacitances; estimated OT and RLC.", generated_by="scripts/spice/nfb_coupled_reference.py"))
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print(f"Wrote {len(rows)} AC and {len(dc_rows)} DC points to {OUT}")


if __name__ == "__main__":
    main()
