"""Genera `assets/fixtures/signals/bass_test.wav` (nota de bajo sintética).

E2 (82.41 Hz) con armónicos 2x y 3x y envolvente exponencial; mono, PCM 16
bits, 48 kHz, 0,5 s. Señal propia del proyecto: no proviene de grabaciones.
"""

import math
import struct
import wave
from pathlib import Path

SAMPLE_RATE_HZ = 48_000
DURATION_S = 0.5
FUNDAMENTAL_HZ = 82.41
DEST = Path(__file__).resolve().parent.parent / "assets/fixtures/signals/bass_test.wav"


def main() -> None:
    frame_count = int(SAMPLE_RATE_HZ * DURATION_S)
    data = bytearray()
    for index in range(frame_count):
        t = index / SAMPLE_RATE_HZ
        envelope = math.exp(-3.0 * t)
        signal = (
            0.6 * math.sin(2.0 * math.pi * FUNDAMENTAL_HZ * t)
            + 0.25 * math.sin(2.0 * math.pi * 2.0 * FUNDAMENTAL_HZ * t)
            + 0.1 * math.sin(2.0 * math.pi * 3.0 * FUNDAMENTAL_HZ * t)
        )
        clamped = max(-1.0, min(1.0, 0.8 * envelope * signal))
        data += struct.pack("<h", int(clamped * 32_767))

    with wave.open(str(DEST), "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(SAMPLE_RATE_HZ)
        wav_file.writeframes(bytes(data))

    print(f"escrito {DEST} ({frame_count} frames)")


if __name__ == "__main__":
    main()
