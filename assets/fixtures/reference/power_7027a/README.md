# Referencia 7027-A (B18-010/011)

`7027a_rca_digitized.csv` contiene puntos digitalizados del datasheet RCA
7027-A (DATA 1/2, 8-59), copia local en `assets/reference/7027A_RCA.pdf`
(fuera de git). Se obtuvo con `scripts/digitize_7027a_curves.py` a partir del
escaneo 1-bit extraído por `scripts/extract_pdfs.py`; el verificador
`scripts/check_reference_manifest.py` comprueba el SHA-256 registrado.

## Figuras y condiciones

| figure | condición | rango | puntos |
|---|---|---|---|
| `p05` | "Average plate characteristics", Vg1 = 0 V, Ec2 = 50…450 V (paso 50 V) | Vp = 60…600 V | 9 curvas |
| `p07` | "Average characteristics", Vp = 300 V, Ec2 = 300 V | Vg1 = −0.5…−37.5 V (Ip) y −0.5…−12.5 V (Ig2) | transferencia |
| `p06` | curva de Ig2 **sin etiqueta legible** (Ec2 = 300 V) | Vp = 12…370 V | solo la forma `Ig2(Vp)` |

El resto de la familia `p06` (Ip/Ig2 con Vg1 como parámetro) **no** se
digitaliza: sus curvas de Ig2 y las de corte profundo se entrelazan junto al
eje y las etiquetas se superponen. El nivel absoluto de la pantalla se fija con
la tabla de operación típica del propio datasheet (páginas 1–3), incorporada
como anclas en `scripts/fit_7027a_model.py`; el `Max.-Signal Grid-No.2 Current`
de esa tabla es un valor medio del ciclo de señal y no se usa como punto.

## Cómo se digitalizó

- Calibración de ejes por las etiquetas impresas (centros de glifo) y la
  rejilla (p05: celdas de 10 V × 10 mA; p06: 5 V; p07: 1 V en x); constantes en
  el script.
- p05: seguimiento de trazos columna a columna sobre la rejilla enmascarada,
  con fusión de fragmentos; la identidad de curvas se ancla con la
  comprobación cruzada independiente `Ip(Ec2=300, Vp=300, Vg1=0)` frente a p07
  (≈247 frente a ≈250 mA).
- p07/p06: muestreo guiado por anclas de trazo leídas del escaneo (±12 px).
- Los overlays de verificación (`extracted/p0x_digitized_overlay.png`) no se
  versionan.

## Incertidumbre declarada

±5 V y ±3 mA en p05 (media celda y grosor de trazo); ±1 V y ±2 mA en p07 (Ip),
±1 mA (Ig2); ±5 V en p06. Son lecturas del escaneo, **no** mediciones de
laboratorio ni valores garantizados por RCA.

## Uso

`scripts/fit_7027a_model.py` consume este CSV más las anclas tabuladas del
datasheet y produce los parámetros Koren de la 7027A (y la división de pantalla
de B18-011) que se registran en `assets/models/b18n_1967_calibration.json`.
El contraste vive en `crates/b18n-dsp/tests/power_7027a_curves.rs`.
