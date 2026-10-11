# Referencias históricas locales

Copia local, **no redistribuible**, de los documentos usados para trazabilidad:

- `b18n-b15nd_schematic_5_67.pdf` — esquema B-18N/B-15ND, mayo de 1967 (rev. `1967-05`).
  Autoridad para **conexiones** y componentes.
- `b18n_1967_service_manual.pdf` — manual de servicio B-18N. Medidas de reposo,
  valores de componentes y recuadro `Printed CKT`.

Los PDF están excluidos de git (`.gitignore`) por derechos de terceros. Para reproducir
el entorno:

```powershell
Invoke-WebRequest -Uri "https://vintage-blue.com/assets/b18n-b15nd_schematic_5_67.pdf" -OutFile "assets/reference/b18n-b15nd_schematic_5_67.pdf"
Invoke-WebRequest -Uri "https://vintage-blue.com/assets/b18n_1967_service_manual.pdf" -OutFile "assets/reference/b18n_1967_service_manual.pdf"
```

Extracción de imágenes para lectura (Python 3; el manual requiere `pypdf`):

```powershell
python scripts/extract_pdfs.py assets/reference extracted
```

Los hallazgos de trazabilidad se documentan en `docs/CIRCUIT_TRACEABILITY.md`.
