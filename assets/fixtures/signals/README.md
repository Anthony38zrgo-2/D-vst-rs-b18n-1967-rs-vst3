# Señales de prueba

Señales generadas por el propio proyecto (senos, multisinus, impulsos, transitorios).
No se aceptan capturas de terceros sin licencia.

## `bass_test.wav`

Nota de bajo sintética: E2 (82.41 Hz) con armónicos 2× y 3×, envolvente
exponencial de 0.5 s, mono, PCM 16 bits, 48 kHz. Generada con el script
`scripts/generate_bass_test.py`; no proviene de ninguna grabación.

Reglas:

- Documentar cada archivo: tipo de señal, frecuencia(s), nivel pico, duración, sample rate
  y el comando o script que lo generó.
- Los artefactos generados se pueden reproducir con `b18n-render` o scripts versionados;
  evitar almacenar WAV grandes en el repositorio.
