"""B18-009 sensitivity harness of the continuous NFB loop (ngspice 47).

Independent SPICE study that isolates, one factor at a time, which element of
the declared continuous circuit produces the negative return-ratio margins
reported in `assets/fixtures/reference/nfb/loop_margins.json`.

It reuses the validated netlist of `nfb_loop_reference.py` (Koren RCA tubes,
frozen rails 380/460/505/500/-55 V, Rmix/2 source, 22 kohm cathode NFB from the
16 ohm GREEN tap) and parametrizes the OT-471 differential equivalent so the
magnetizing inductance, leakage, parasitic capacitance, core loss, databook
interelectrode capacitances, secondary load, tap polarity and source impedance
can each be enabled/disabled/swept in isolation.

Method: break only the unloaded voltage sensing tap, inject 1 V AC with the DC
operating point unchanged, T = -Vreturn/Vtest = polarity*(tapgain)*V(s4).
Characteristic equation 1+T. All 0 dB and -180/-540/-900 deg crossings are
reported, never just the first. Transients use a 10 uV pulse retired at 2.1 us.

Writes new fixtures (`sensitivity_*.csv/json`) under
`assets/fixtures/reference/nfb/`; the historical fixtures are never overwritten.
Register them with `scripts/check_reference_manifest.py` after generating.

Uso:
    python scripts/spice/nfb_sensitivity.py              # margenes de todos los escenarios
    python scripts/spice/nfb_sensitivity.py --transient  # anade transitorios clave
    python scripts/spice/nfb_sensitivity.py --passivity  # solo chequeo de pasividad
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import re
from pathlib import Path

import nfb_loop_reference as reference

REPO_ROOT = reference.REPO_ROOT
OUT = REPO_ROOT / "assets" / "fixtures" / "reference" / "nfb"
WORK = reference.WORK
NGSPICE = reference.NGSPICE

N8 = reference.N8
N16 = reference.N16
TAP16_GAIN = reference.TAP_GAIN  # N8/N16 = 1.414, tap de 16 ohm sobre el de 8 ohm
TAP8_GAIN = 1.0

# Escenarios de transitorio (los mas lentos); el resto solo AC.
TRANSIENT_SCENARIOS = {
    "baseline",
    "ideal",
    "cp_0p",
    "cp_470p",
    "lleak_6m",
    "lleak_48m",
    "rcore_0",
    "rcore_2000",
    "no_caps",
    "inverted_polarity",
    "corrected_cp1500p",
    "corrected_cp1500p_rw900",
}

CAPS_LINES = [
    "cgk_p pin7 pin6 5p",
    "cgp_p pin7 pin2 0.06p",
    "cpk_p pin2 pin6 2p",
    "cgp_t pin9 pin1 2p",
    "cgk_t pin9 pin8 2.3p",
    "cpk_t pin1 pin8 0.3p",
    "cg1p_1 g1 pa 1.5p",
    "cg1k_1 g1 0 10p",
    "cpk_1 pa 0 7.5p",
    "cg1p_2 g2 pb 1.5p",
    "cg1k_2 g2 0 10p",
    "cpk_2 pb 0 7.5p",
]


def subckts() -> str:
    """Koren subcircuits with the 7199 parameters from the calibration overlay."""
    text = reference.SUBCKTS
    calibration = json.loads(
        (REPO_ROOT / "assets/models/b18n_1967_calibration.json").read_text(
            encoding="utf-8"
        )
    )["assumptions"]
    for tube, subckt in [("7199.pentode", "k7199p"), ("7199.triode", "k7199t")]:
        prefix = f"tube_models.{tube}.koren."
        params = {
            key[len(prefix) :]: entry["value"]
            for key, entry in calibration.items()
            if key.startswith(prefix)
        }
        declaration = re.search(rf"\.subckt {subckt} .*", text).group()
        for key, value in params.items():
            declaration = re.sub(rf"\b{key}=[^ ]+", f"{key}={value:.15g}", declaration)
        text = re.sub(rf"\.subckt {subckt} .*", declaration, text)
    return text


def ot_block(ot, lm, lleak, cp, rcore, r_wind=0.0) -> list[str]:
    """OT-471 differential equivalent with selectable elements.

    Nodes: `d` is the differential-mode voltage v(pa)-v(pb); `a` is the
    differential network input after the primary DCR; `f` is the flux node
    (after the core-loss resistance); `hw` is the leakage winding node; `g` is
    the secondary-side node after the leakage inductance.

    `r_wind` is an optional winding AC resistance in series with the leakage
    inductance (the HF loss that the declared topology places only in the
    magnetizing branch).
    """
    lines = [
        "* Common-mode half-primary DCR, 150 ohms per side.",
        "Bcommon1 pa bpl I = {(v(pa)+v(pb)-2*v(bpl))/300}",
        "Bcommon2 pb bpl I = {(v(pa)+v(pb)-2*v(bpl))/300}",
        "Ed d 0 pa pb 1",
        "Fdiff1 pa 0 Ed -1",
        "Fdiff2 pb 0 Ed 1",
        "Rdc d a 300",
    ]
    if ot == "ideal":
        # Ideal transformer: no magnetizing, leakage, parasitic or core loss.
        lines += [
            f"esec s1 0 a 0 {{1/{N8}}}",
            "vsense s1 s2 0",
            "rdsec s2 s4 0.4",
            f"fpri a 0 vsense {{1/{N8}}}",
        ]
        return lines
    node = "a"
    if rcore is not None:
        lines.append(f"Rcore a f {rcore:.6g}")
        node = "f"
    if lm is not None:
        lines.append(f"Lm {node} 0 {lm:.6g}")
    if lleak is not None:
        leak_node = node
        if r_wind > 0.0:
            lines.append(f"Rw {node} hw {r_wind:.6g}")
            leak_node = "hw"
        lines.append(f"Ll {leak_node} g {lleak:.6g}")
        secondary = "g"
    else:
        secondary = node
    if cp is not None:
        lines.append(f"Cp {node} 0 {cp:.6g}")
    lines += [
        f"esec s1 0 {secondary} 0 {{1/{N8}}}",
        "vsense s1 s2 0",
        "rdsec s2 s4 0.4",
        f"fpri {secondary} 0 vsense {{1/{N8}}}",
    ]
    return lines


def load_lines(load: str) -> list[str]:
    if load == "nominal":
        return ["rload s4 0 8"]
    return [
        "re s4 s5 6.5",
        "le s5 m 0.8m",
        "res m 0 23.5",
        "les m 0 0.0693",
        "ces m 0 180.6u",
    ]


def tone_network(suffix: str, input_node: str) -> list[str]:
    """James/Baxandall tone network of one channel at the 50 % pot setting.

    Mirrors `tone_stack_coupled_sweep.py` (Ro = 100 kohm of the second 6SL7,
    0.022 uF coupling, pots of 1 Mohm at 50 %). Used to build the real complex
    source impedance seen at the 7199 grid: one active channel plus the
    inactive channel, each through its 270 kohm mixer resistor.
    """
    s = suffix
    return [
        f"rt_src{s} {input_node} tsrc{s} 100k",
        f"ccoup{s} tsrc{s} in{s} 0.022u",
        f"rser{s} in{s} b{s} 220k",
        f"ct1{s} in{s} t1{s} 0.00047u",
        f"cbtop{s} b{s} w{s} 0.001u",
        f"cbbot{s} c{s} w{s} 0.01u",
        f"rbw{s} b{s} w{s} 500k",
        f"rwc{s} w{s} c{s} 500k",
        f"rstp{s} c{s} 0 22k",
        f"rcut{s} w{s} 0 270k",
        f"rmid{s} w{s} t1{s} 120k",
        f"rtout{s} t1{s} o{s} 500k",
        f"rout2{s} o{s} t2{s} 500k",
        f"ct2{s} t2{s} 0 0.0047u",
        f"rmix{s} o{s} pin7 270k",
    ]


def netlist(
    *,
    load: str,
    scenario: str,
    ot: str = "linear",
    lm: float | None = 28.7,
    lleak: float | None = 12e-3,
    cp: float | None = 100e-12,
    rcore: float | None = 500.0,
    r_wind: float = 0.0,
    caps: bool = True,
    source_ohms: float = 135e3,
    source_tone: bool = False,
    tap: str = "16",
    polarity: int = 1,
    measure: str = "return",
) -> str:
    """Build the continuous loop netlist.

    `measure`:
      - "return": break the sensing tap, inject 1 V AC, vin = 0 (loop gain).
      - "closed": keep the physical Btp sensor and drive vin (loop response).

    `source_tone`: replace the 135 kohm Thevenin source by the real complex
    impedance of the two tone networks (active channel + inactive channel)
    through their 270 kohm mixer resistors.
    """
    tap_gain = TAP16_GAIN if tap == "16" else TAP8_GAIN
    sensor = f"Btp tp 0 V = {{{-polarity * tap_gain:.6f}*v(s4)}}"
    if measure == "return":
        sensor = "Vtest tp 0 DC 0 AC 1"
        drive = "vin inx 0 dc 0 ac 0"
    else:
        drive = "vin inx 0 dc 0 ac 1"
    if source_tone:
        source_lines = tone_network("a", "inx") + tone_network("b", "0")
    else:
        source_lines = [f"rsrc inx pin7 {source_ohms:g}"]
    lines = [
        f"* B-18N 1967 sensitivity scenario={scenario} load={load}",
        f"* ot={ot} lm={lm} lleak={lleak} cp={cp} rcore={rcore} rwind={r_wind} caps={caps}",
        "vb rail_p 0 dc 380",
        "vt rail_t 0 dc 460",
        "vbias bias 0 dc -55",
        "vscr scr 0 dc 500",
        "vbp bpl 0 dc 505",
        drive,
    ]
    lines += source_lines
    lines += [
        "* no untraced grid-leak resistor (see nfb_coupled_reference.py)",
        "r270 rail_p pin2 270k",
        "r470 rail_p pin3 470k",
        "r150 pin3 0 150k",
        "c01 pin3 pin6 0.1u",
        "r15 pin6 0 1.5k",
        "c02 pin2 pin9 0.02u",
        "r10m pin1 pin9 10meg",
        "r33m pin9 0 3.3meg",
        "r47p rail_t pin1 47k",
        "r47k pin8 0 47k",
        "rfb tp pin6 22000",
        sensor,
        "xpent pin2 pin3 pin7 pin6 k7199p",
        "xtri pin1 pin9 pin8 k7199t",
        "cg1 pin1 n1 50n",
        "rs1 n1 g1 1k",
        "rg1 g1 bias 100k",
        "cg2 pin8 n2 50n",
        "rs2 n2 g2 1k",
        "rg2 g2 bias 100k",
        "xp1 pa scr g1 0 k7027a",
        "xp2 pb scr g2 0 k7027a",
    ]
    lines += ot_block(ot, lm, lleak, cp, rcore, r_wind)
    lines += load_lines(load)
    if caps:
        lines += CAPS_LINES
    lines += [subckts(), ".op", ".options method=gear"]
    return "\n".join(lines) + "\n"


def control_return(name: str) -> str:
    return f""".control
set numdgt=15
op
ac dec 200 1 100meg
wrdata {(WORK / f"ret_{name}.txt").as_posix()} vr(s4) vi(s4)
.endc
.end
"""


def control_transient(path: Path) -> str:
    return f""".control
set numdgt=12
tran 0.1u 2m 0 0.1u
wrdata {path.as_posix()} v(s4)
.endc
.end
"""


def run(name: str, body: str, control: str) -> str:
    text = body + control
    log, status = reference.run(name, text)
    if status != "0" or "Error" in log:
        raise RuntimeError(f"{name}: rc={status}\n{log[-2000:]}")
    return name


def summarize_return(name: str, tap_gain: float, polarity: int) -> dict:
    sign = polarity * tap_gain
    rows = []
    previous = 0.0
    for line in (WORK / f"ret_{name}.txt").read_text().splitlines():
        fields = line.split()
        if len(fields) < 4:
            continue
        frequency, real, _, imag = map(float, fields[:4])
        real *= sign
        imag *= sign
        phase = math.degrees(math.atan2(imag, real))
        while phase - previous > 180:
            phase -= 360
        while phase - previous < -180:
            phase += 360
        previous = phase
        rows.append((frequency, 20 * math.log10(math.hypot(real, imag)), phase))
    if not rows:
        raise RuntimeError(f"{name}: sin datos de retorno")
    anchor = min(rows, key=lambda row: abs(math.log(row[0] / 1000)))
    shift = 360 * round(anchor[2] / 360)
    rows = [(f, g, p - shift) for f, g, p in rows]
    unity, phase_crossings = [], []
    for a, b in zip(rows, rows[1:]):
        if a[1] * b[1] < 0:
            weight = -a[1] / (b[1] - a[1])
            phase = a[2] + weight * (b[2] - a[2])
            unity.append(
                dict(
                    frequency_hz=a[0] * (b[0] / a[0]) ** weight,
                    direction="falling" if b[1] < a[1] else "rising",
                    phase_margin_deg=180 + phase,
                    gain_db=0.0,
                )
            )
        for boundary in (-180, -540, -900):
            if (a[2] - boundary) * (b[2] - boundary) < 0:
                weight = (boundary - a[2]) / (b[2] - a[2])
                gain = a[1] + weight * (b[1] - a[1])
                phase_crossings.append(
                    dict(
                        frequency_hz=a[0] * (b[0] / a[0]) ** weight,
                        boundary_deg=boundary,
                        gain_margin_db=-gain,
                    )
                )
    falling = [c for c in unity if c["direction"] == "falling"]
    hf = max(falling, key=lambda c: c["frequency_hz"]) if falling else None
    negative = [c for c in phase_crossings if c["gain_margin_db"] < 0]
    critical = min(negative, key=lambda c: c["frequency_hz"]) if negative else None
    return dict(
        unity_gain_crossings=unity,
        phase_crossings=phase_crossings,
        hf_unity_hz=hf["frequency_hz"] if hf else None,
        hf_phase_margin_deg=hf["phase_margin_deg"] if hf else None,
        worst_gain_margin_db=critical["gain_margin_db"] if critical else None,
        gain_margin_hz=critical["frequency_hz"] if critical else None,
    )


def all_scenarios() -> list[dict]:
    scenarios: list[dict] = []

    def add(name, **kwargs):
        scenarios.append(dict(name=name, **kwargs))

    # 1. Incremental OT isolation.
    add("ideal", ot="ideal", lm=None, lleak=None, cp=None, rcore=None)
    add("lm_only", ot="linear", lm=28.7, lleak=None, cp=None, rcore=None)
    add("lm_lleak", ot="linear", lm=28.7, lleak=12e-3, cp=None, rcore=None)
    add("lm_lleak_cp", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12, rcore=None)
    add("baseline", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12, rcore=500.0)
    # 2. One-at-a-time sweeps from the baseline.
    for value in [0.0, 22e-12, 47e-12, 220e-12, 470e-12]:
        add(
            f"cp_{value * 1e12:.0f}p",
            ot="linear", lm=28.7, lleak=12e-3,
            cp=(None if value == 0.0 else value), rcore=500.0,
        )
    for value in [6e-3, 24e-3, 48e-3]:
        add(f"lleak_{value * 1e3:.0f}m", ot="linear", lm=28.7, lleak=value, cp=100e-12, rcore=500.0)
    for value in [0.0, 100.0, 2000.0]:
        add(f"rcore_{value:.0f}", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12,
            rcore=(None if value == 0.0 else value))
    for value in [14.0, 57.0]:
        add(f"lm_{value:.0f}h", ot="linear", lm=value, lleak=12e-3, cp=100e-12, rcore=500.0)
    # 3. Databook capacitances and source impedance.
    add("no_caps", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12, rcore=500.0, caps=False)
    for ohms in [220e3, 270e3, 330e3]:
        add(f"source_{ohms / 1e3:.0f}k", ot="linear", lm=28.7, lleak=12e-3,
            cp=100e-12, rcore=500.0, source_ohms=ohms)
    add("source_tone", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12,
        rcore=500.0, source_tone=True)
    # 4. Polarity and tap.
    add("inverted_polarity", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12,
        rcore=500.0, polarity=-1)
    add("tap8", ot="linear", lm=28.7, lleak=12e-3, cp=100e-12, rcore=500.0, tap="8")
    # 5. Proposed corrections: the estimated Cp=100 pF puts the leakage
    # resonance at ~145 kHz, inside the loop crossover. A physically bounded
    # total shunt capacitance places the OT HF resonance at ~37 kHz (typical of
    # a 1967 50 W push-pull transformer) and restores positive margins.
    add("corrected_cp1500p", ot="linear", lm=28.7, lleak=12e-3, cp=1.5e-9, rcore=500.0)
    add("corrected_cp1500p_rw900", ot="linear", lm=28.7, lleak=12e-3, cp=1.5e-9,
        rcore=500.0, r_wind=900.0)
    return scenarios


def run_margins(args) -> dict:
    results = []
    for scenario in all_scenarios():
        for load in ["nominal", "reactiva"]:
            name = f"sens_{scenario['name']}_{load}"
            tap_gain = TAP16_GAIN if scenario.get("tap", "16") == "16" else TAP8_GAIN
            body = netlist(scenario=scenario["name"], load=load, **{
                key: value for key, value in scenario.items() if key != "name"
            })
            run(name, body, control_return(name))
            summary = summarize_return(name, tap_gain, scenario.get("polarity", 1))
            record = dict(scenario=scenario["name"], load=load)
            record.update({key: value for key, value in scenario.items() if key != "name"})
            record.update(summary)
            results.append(record)
            print(
                f"{scenario['name']:>18} {load:>8}: HF cruce "
                f"{_fmt(record['hf_unity_hz'], 'Hz')}, MF "
                f"{_fmt(record['hf_phase_margin_deg'], 'deg')}, MG "
                f"{_fmt(record['worst_gain_margin_db'], 'dB')} @ "
                f"{_fmt(record['gain_margin_hz'], 'Hz')}"
            )
    return results


def _fmt(value, unit):
    if value is None:
        return "n/a"
    if unit == "Hz":
        return f"{value / 1e3:.1f} kHz"
    if unit == "deg":
        return f"{value:+.2f} deg"
    return f"{value:+.2f} dB"


def run_passivity() -> list[dict]:
    """Differential one-port impedance; passive requires Re(Z) >= 0."""
    rows = []
    for scenario in all_scenarios():
        for load in ["nominal", "reactiva"]:
            name = f"pass_{scenario['name']}_{load}"
            lines = [
                f"* passividad OT escenario={scenario['name']} load={load}",
                "vbp bpl 0 dc 505",
                "Iap 0 pa AC 1",
                "Ibp 0 pb AC -1",
            ]
            lines += ot_block(
                scenario.get("ot", "linear"),
                scenario.get("lm", 28.7),
                scenario.get("lleak", 12e-3),
                scenario.get("cp", 100e-12),
                scenario.get("rcore", 500.0),
                scenario.get("r_wind", 0.0),
            )
            lines += load_lines(load)
            control = f""".control
set numdgt=15
ac dec 100 1 100meg
wrdata {(WORK / f'z_{name}.txt').as_posix()} v(pa) v(pb)
.endc
.end
"""
            run(name, "\n".join(lines) + "\n", control)
            min_re, at_hz = None, None
            for line in (WORK / f"z_{name}.txt").read_text().splitlines():
                fields = line.split()
                if len(fields) < 6:
                    continue
                frequency, pa_re, pa_im, _, pb_re, pb_im = map(float, fields[:6])
                z_re = pa_re - pb_re
                if min_re is None or z_re < min_re:
                    min_re, at_hz = z_re, frequency
            rows.append(
                dict(
                    scenario=scenario["name"],
                    load=load,
                    min_re_z_ohm=min_re,
                    at_hz=at_hz,
                    passive=min_re is not None and min_re >= -1e-6,
                )
            )
            print(
                f"pasividad {scenario['name']:>18} {load:>8}: "
                f"min Re(Z) {min_re:+.3e} ohm @ {at_hz:.1f} Hz "
                f"-> {'PASIVO' if min_re >= -1e-6 else 'NO PASIVO'}"
            )
    return rows


def run_transients() -> list[dict]:
    rows = []
    for scenario in all_scenarios():
        if scenario["name"] not in TRANSIENT_SCENARIOS:
            continue
        for load in ["nominal", "reactiva"]:
            name = f"tr_{scenario['name']}_{load}"
            body = netlist(scenario=scenario["name"], load=load, measure="closed", **{
                key: value for key, value in scenario.items() if key != "name"
            })
            body = body.replace(
                "vin inx 0 dc 0 ac 1",
                "vin inx 0 PWL(0 0 1u 0 1.1u 0.00001 2u 0.00001 2.1u 0)",
            )
            output = WORK / f"trans_{name}.txt"
            text = body + control_transient(output)
            log, status = reference.run(name, text)
            if status != "0" or "Error" in log:
                raise RuntimeError(f"{name}: rc={status}\n{log[-2000:]}")
            samples = []
            for line in output.read_text().splitlines():
                fields = line.split()
                if len(fields) >= 2:
                    samples.append((float(fields[0]), float(fields[1])))
            output.unlink(missing_ok=True)
            if not samples:
                rows.append(dict(scenario=scenario["name"], load=load, aborted=True, windows=[]))
                print(f"transitorio {scenario['name']:>18} {load:>8}: ABORTADO (sin capacitancia de nodo)")
                continue
            windows = []
            for low, high in [(0, 1e-5), (1e-4, 2e-4), (1e-3, 2e-3)]:
                peak = max((abs(v) for t, v in samples if low <= t < high), default=0.0)
                windows.append(dict(start_seconds=low, end_seconds=high, peak_v=peak))
            rows.append(dict(scenario=scenario["name"], load=load, windows=windows))
            print(
                f"transitorio {scenario['name']:>18} {load:>8}: "
                f"picos {windows[0]['peak_v']:.2e} / {windows[1]['peak_v']:.4f} / "
                f"{windows[2]['peak_v']:.4f} V"
            )
    return rows


def write_json(path: Path, payload: dict) -> None:
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"escrito {path}")


def write_margins_csv(path: Path, rows: list[dict]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "scenario", "load", "ot", "lm_h", "lleak_h", "cp_f", "rcore_ohm",
                "r_wind_ohm", "caps", "source_ohms", "tap", "polarity", "hf_unity_hz",
                "hf_phase_margin_deg", "worst_gain_margin_db", "gain_margin_hz",
            ]
        )
        for row in rows:
            writer.writerow(
                [
                    row["scenario"], row["load"], row.get("ot"), row.get("lm"),
                    row.get("lleak"), row.get("cp"), row.get("rcore"),
                    row.get("r_wind", 0.0), row.get("caps", True),
                    row.get("source_ohms", 135e3), row.get("tap", "16"),
                    row.get("polarity", 1), row.get("hf_unity_hz"),
                    row.get("hf_phase_margin_deg"), row.get("worst_gain_margin_db"),
                    row.get("gain_margin_hz"),
                ]
            )
    print(f"escrito {path}")


def update_manifest(entries: list[tuple[Path, str]]) -> None:
    manifest_path = OUT.parent / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for path, description in entries:
        relative = f"nfb/{path.name}"
        manifest["entries"] = [
            entry for entry in manifest["entries"] if entry["path"] != relative
        ]
        manifest["entries"].append(
            dict(
                path=relative,
                provenance="independent-SPICE",
                sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                description=description,
                conditions=(
                    "ngspice 47; frozen rails 380/460/505/500/-55 V; Rmix/2 source; "
                    "unloaded 16 ohm sensing tap injection, 1 Hz to 100 MHz."
                ),
                generated_by="scripts/spice/nfb_sensitivity.py",
            )
        )
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"manifiesto actualizado ({len(entries)} entradas)")


def generate_corrected_reference(cp: float = 1.5e-9) -> tuple[Path, Path]:
    """Coupled AC reference with the corrected OT shunt capacitance.

    Reuses the validated `nfb_coupled_reference.netlist` (same frozen rails,
    ports and tubes) and only substitutes the estimated `Cp a 0 100p` by the
    corrected value, so the isolated Rust candidate can be compared against a
    reference consistent with the corrected continuous model.
    """
    import nfb_coupled_reference as coupled_ref

    csv_path = OUT / "coupled_ac_corrected.csv"
    dc_path = OUT / "coupled_dc_corrected.csv"
    ac_rows, dc_rows = [], []
    for load in ["nominal", "reactiva"]:
        for feedback in [False, True]:
            text = coupled_ref.netlist(load, feedback).replace(
                "Cp a 0 100p", f"Cp a 0 {cp:g}"
            )
            reference.run(f"corr_ref_{load}_{feedback}", text)
            for index, frequency in enumerate(coupled_ref.FREQUENCIES):
                fields = (
                    reference.WORK / f"point_{index}.txt"
                ).read_text().splitlines()[-1].split()
                magnitude, phase = float(fields[1]), float(fields[3])
                ac_rows.append(
                    [load, int(feedback), frequency, 20 * math.log10(magnitude),
                     math.degrees(phase)]
                )
            fields = (
                reference.WORK / f"dc_{load}_{feedback}.txt"
            ).read_text().splitlines()[-1].split()
            dc_rows.append([load, int(feedback)] + [float(v) for v in fields[1::2]])
    with csv_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["load", "feedback", "frequency_hz", "gain_db", "phase_deg"])
        writer.writerows(ac_rows)
    with dc_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            ["load", "feedback", "g", "p", "s", "k", "tg", "tp", "tk", "u", "l", "a", "b"]
        )
        writer.writerows(dc_rows)
    print(f"escrito {csv_path} ({len(ac_rows)} puntos AC, Cp={cp:g})")
    return csv_path, dc_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--transient", action="store_true", help="incluye transitorios clave")
    parser.add_argument("--passivity", action="store_true", help="incluye chequeo de pasividad")
    parser.add_argument("--all", action="store_true", help="margenes + transitorios + pasividad")
    parser.add_argument(
        "--corrected-reference", action="store_true",
        help="genera la referencia acoplada con el Cp corregido (1,5 nF)",
    )
    args = parser.parse_args()
    if not NGSPICE.exists():
        raise SystemExit(f"no encontrado: {NGSPICE}")

    if args.corrected_reference:
        ac_path, dc_path = generate_corrected_reference()
        update_manifest(
            [
                (ac_path, "B18-009 coupled output network AC with corrected OT Cp=1.5 nF."),
                (dc_path, "B18-009 coupled output network DC with corrected OT Cp=1.5 nF."),
            ]
        )
        return 0

    generated: list[tuple[Path, str]] = []
    payload: dict = {
        "method": (
            "Continuous return ratio by breaking the unloaded sensing tap, 1 V AC "
            "injection, frozen rails, 1 Hz to 100 MHz, ngspice 47."
        ),
        "note": (
            "No se ajustan parametros para pasar un test; cada escenario aisla un "
            "factor del equivalente del OT-471 declarado (valores estimated del overlay)."
        ),
    }

    margins = run_margins(args)
    payload["margins"] = margins
    margins_json = OUT / "sensitivity_margins.json"
    write_json(margins_json, payload)
    margins_csv = OUT / "sensitivity_margins.csv"
    write_margins_csv(margins_csv, margins)
    generated += [
        (margins_json, "B18-009 sensitivity of the continuous return ratio, one OT factor at a time."),
        (margins_csv, "B18-009 sensitivity headline margins per scenario and load."),
    ]

    if args.passivity or args.all:
        passivity = run_passivity()
        passivity_json = OUT / "sensitivity_passivity.json"
        write_json(passivity_json, {"method": "Differential one-port, Re(Z) >= 0 required.", "scenarios": passivity})
        generated.append(
            (passivity_json, "B18-009 differential one-port passivity check of the OT-471 equivalent.")
        )

    if args.transient or args.all:
        transients = run_transients()
        transient_json = OUT / "sensitivity_transients.json"
        write_json(transient_json, {"method": "10 uV pulse retired at 2.1 us, max step 0.1 us.", "scenarios": transients})
        generated.append(
            (transient_json, "B18-009 10 uV impulse transients of the continuous loop.")
        )

    update_manifest(generated)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
