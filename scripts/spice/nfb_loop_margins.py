"""Independent return-ratio measurement; keeps the 22k cathode loading.

Break only the unloaded voltage sensing tap and inject 1 V AC, DC 0 V.
T = -Vreturn/Vtest = (N8/N16)*Vsecondary; characteristic equation 1+T.
All unity-gain and -180 degree crossings are reported, not just the first.
"""
import json
import math
import hashlib
import nfb_coupled_reference as coupled
import nfb_loop_reference as reference


def margins(load):
    text = coupled.netlist(load, True)
    text = text.replace("vin inx 0 dc 0 ac 1", "vin inx 0 dc 0 ac 0")
    text = text.replace(f"Btp tp 0 V = {{-{reference.TAP_GAIN:.6f}*v(s4)}}", "Vtest tp 0 DC 0 AC 1")
    text = text[:text.index(".control")] + f""".control
set numdgt=15
ac dec 200 1 100meg
wrdata {reference.WORK.as_posix()}/return_ratio.txt vr(s4) vi(s4)
.endc
.end
"""
    log, status = reference.run(f"return_ratio_{load}", text)
    if status != "0" or "Error" in log:
        raise RuntimeError(log)
    rows = []
    previous = 0.0
    for line in (reference.WORK/"return_ratio.txt").read_text().splitlines():
        f, real, _, imag = map(float, line.split())
        real *= reference.TAP_GAIN
        imag *= reference.TAP_GAIN
        phase = math.degrees(math.atan2(imag, real))
        while phase-previous > 180: phase -= 360
        while phase-previous < -180: phase += 360
        previous = phase
        rows.append((f, 20*math.log10(math.hypot(real,imag)),phase))
    anchor=min(rows,key=lambda row: abs(math.log(row[0]/1000)))
    shift=360*round(anchor[2]/360)
    rows=[(f,g,p-shift) for f,g,p in rows]
    unity, phase_crossings = [], []
    for a,b in zip(rows,rows[1:]):
        if a[1]*b[1] < 0:
            weight=-a[1]/(b[1]-a[1])
            phase=a[2]+weight*(b[2]-a[2])
            unity.append(dict(frequency_hz=a[0]*(b[0]/a[0])**weight, direction="falling" if b[1]<a[1] else "rising", phase_margin_deg=180+phase))
        for boundary in [-180,-540,-900]:
            if (a[2]-boundary)*(b[2]-boundary) < 0:
                weight=(boundary-a[2])/(b[2]-a[2])
                gain=a[1]+weight*(b[1]-a[1])
                phase_crossings.append(dict(frequency_hz=a[0]*(b[0]/a[0])**weight,gain_margin_db=-gain))
    return dict(load=load,unity_gain_crossings=unity,phase_crossings=phase_crossings)


def transient(load):
    text=coupled.netlist(load,True).replace("vin inx 0 dc 0 ac 1", "vin inx 0 PWL(0 0 1u 0 1.1u 0.00001 2u 0.00001 2.1u 0)")
    path=reference.WORK/f"transient_{load}.txt"
    text=text[:text.index(".control")]+f""".control
set numdgt=12
tran 0.1u 2m 0 0.1u
wrdata {path.as_posix()} v(s4)
.endc
.end
"""
    log,status=reference.run(f"transient_{load}",text)
    if status != "0" or "Error" in log: raise RuntimeError(log)
    samples=[list(map(float,line.split())) for line in path.read_text().splitlines()]
    windows=[]
    for low,high in [(0,1e-5),(1e-4,2e-4),(1e-3,2e-3)]:
        peak=max(abs(row[1]) for row in samples if low<=row[0]<high)
        windows.append(dict(start_seconds=low,end_seconds=high,peak_secondary_v=peak))
    return dict(load=load,pulse_volts=1e-5,max_timestep_seconds=1e-7,windows=windows,
                limitation="Reference tube grid-conduction branches omitted; large-signal amplitudes are diagnostic, not a hardware prediction.")


if __name__ == "__main__":
    result=dict(method="Unloaded sensing tap injection; continuous AC, frozen rails, 1 Hz to 100 MHz, ngspice 47",results=[margins(load) for load in ["nominal","reactiva"]],transients=[transient(load) for load in ["nominal","reactiva"]])
    destination=coupled.OUT/"loop_margins.json"
    destination.write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    manifest_path=coupled.OUT.parent/"manifest.json"
    manifest=json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["entries"]=[entry for entry in manifest["entries"] if entry["path"] != "nfb/loop_margins.json"]
    manifest["entries"].append(dict(path="nfb/loop_margins.json",provenance="independent-SPICE",sha256=hashlib.sha256(destination.read_bytes()).hexdigest(),description="B18-009 negative return-ratio margins and unstable 10 uV impulse; candidate must not ship.",conditions="ngspice 47; frozen rails, unloaded sensing; AC 1 Hz to 100 MHz; transient max step 0.1 us.",generated_by="scripts/spice/nfb_loop_margins.py"))
    manifest_path.write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(result,indent=2))
