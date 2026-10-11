# Referencias SPICE (ngspice) del B-18N 1967

## Política

- Los netlists de esta carpeta son referencias **independientes** del código
  Rust: no deben llamar al solver del proyecto ni copiar sus resultados.
- Solo se ejecutan netlists revisados en este repositorio. Los modelos de
  válvula de terceros requieren revisión de licencia y de procedencia antes de
  incorporarse; se registran fórmula, origen, rango de ajuste y condiciones.
- Todo resultado golden guarda: netlist, versión del simulador, condiciones
  (fuente, carga, controles, temperatura), tolerancias y fecha.
- Los PDF históricos y `tools/` no se versionan; los netlists y sus salidas de
  texto sí.

## Instalación de ngspice 47 (Windows x64, local)

1. Descargar el binario oficial `ngspice-47_64.7z` desde
   <https://sourceforge.net/projects/ngspice/files/ng-spice-rework/47/>.
   SHA-256 del archivo descargado en esta máquina:
   `59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a`.
2. Extraer a `tools/ngspice/` (carpeta ignorada por git). `tar.exe` puede fallar
   con este archivo; usar `7zr.exe` (o 7-Zip):

   ```powershell
   tools\7zr.exe x tools\ngspice-47_64.7z -otools\ngspice -y
   ```

3. El ejecutable de consola es
   `tools/ngspice/Spice64/bin/ngspice_con.exe` (el `ngspice.exe` abre la GUI y
   no imprime en consola). Comprobación:

   ```powershell
   & tools\ngspice\Spice64\bin\ngspice_con.exe -v
   ```

   Debe informar `ngspice-47`.

## Uso

```powershell
python scripts/spice/run_spice.py scripts/spice/b18n_tone_1967.cir
```

El runner busca `ngspice_con.exe` en `tools/ngspice/Spice64/bin/`, acepta
`NGSPICE_BIN` y escribe el log junto al netlist (o en `--log`). Devuelve fallo
si ngspice reporta errores.

## Convenciones de netlist

- Primera línea: título identificable; comentarios con `*`.
- Nodos en minúsculas y con el nombre de la red del esquema (p. ej. `bplus`,
  `pin3`, `gnd`); `0` es la masa.
- Cada netlist indica en comentarios su fuente (`SCH-567`, `SM-1967`), las
  condiciones de simulación y las tolerancias esperadas.
- Para medidas de resistencia equivalente se anulan las fuentes independientes
  (tensión -> cortocircuito, corriente -> abierto) y se inyecta 1 A entre los
  nodos, leyendo la tensión resultante.

## Netlists

| Archivo | Propósito | Estado |
|---|---|---|
| `b18n_tone_1967.cir` | Red de tono (referencia legible al 50 %); barrido completo con `tone_stack_sweep.py` | Verificado |
| `b18n_7199_dc_1967.cir` | `R(pin3, masa)` de la 7199 con fuentes anuladas (apagado/contraste) | Verificado |
| `b18n_7199_meter_loading.cir` | Reposo DC completo de V3 (Koren RCA) y carga del medidor 1/2/5/10 MΩ en pin 9 | Verificado |
| `b18n_7199_nfb_dc_1967.cir` | Reposo DC con la NFB (22 kΩ de la toma de 16 Ω al cátodo del pentodo) | Verificado |
| `b18n_7199_ac_1967.cir` | Respuesta AC del inversor (10 frecuencias); golden con `phase_inverter_reference.py` | Verificado |
| `b18n_7199_tran_1967.cir` | Transitorio 1 kHz/0.2 V del inversor (extremos en 15–20 ms) | Verificado |
| `b18n_power_7027a_1967.cir` | Etapa 7027A / OT-471 (curvas originales si se obtienen) | Pendiente (P2) |

Barrer el tone stack y regenerar el golden:

```powershell
python scripts/spice/tone_stack_sweep.py
```

Las salidas golden se guardan en `assets/fixtures/reference/<tema>/` con un
README que documenta versión, condiciones y tolerancias:

- `assets/fixtures/reference/tone_stack/tone_stack_spice.csv`
- `assets/fixtures/reference/phase_7199/b18n_7199_dc_1967_golden.txt`
- `assets/fixtures/reference/phase_7199/b18n_7199_meter_loading_golden.txt`
- `assets/fixtures/reference/phase_7199/b18n_7199_ac_1967_golden.csv`
- `assets/fixtures/reference/phase_7199/b18n_7199_tran_1967_golden.txt`
