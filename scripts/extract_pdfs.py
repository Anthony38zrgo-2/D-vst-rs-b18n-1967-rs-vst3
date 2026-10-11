"""Extrae imágenes embebidas de los PDF históricos para su lectura y trazabilidad.

- Esquema 5/67: una sola página con un XObject de imagen raster (FlateDecode).
- Manual de servicio: cifrado AESV2 con contraseña de usuario vacía.

Requiere `pypdf` y `Pillow` (pip install pypdf pillow) para descifrar y convertir las
imágenes a PNG. El método inline (zlib) queda como respaldo sin dependencias.

Uso:
    python scripts/extract_pdfs.py assets/reference extracted

Los artefactos extraídos son para análisis interno; no se versionan.
"""

from __future__ import annotations

import re
import sys
import zlib
from pathlib import Path

JPEG_MAGIC = b"\xff\xd8\xff"


def extract_inline_images(pdf_path: Path, out_dir: Path) -> list[Path]:
    """Extrae streams de imagen JPEG comprimidos con zlib de un PDF sin cifrar."""
    data = pdf_path.read_bytes()
    written: list[Path] = []
    index = 0
    for match in re.finditer(rb"stream\r?\n", data):
        start = match.end()
        end = data.find(b"endstream", start)
        if end == -1:
            continue
        raw = data[start:end].rstrip(b"\r\n")
        try:
            decoded = zlib.decompress(raw)
        except zlib.error:
            continue
        if decoded.startswith(JPEG_MAGIC):
            index += 1
            dest = out_dir / f"{pdf_path.stem}_image{index}.jpg"
            dest.write_bytes(decoded)
            written.append(dest)
    return written


def extract_with_pypdf(pdf_path: Path, out_dir: Path) -> list[Path]:
    """Extrae las imágenes de un PDF (cifrado con contraseña vacía) usando pypdf."""
    try:
        from pypdf import PdfReader
    except ImportError:
        print(
            f"[skip] {pdf_path.name}: se necesita pypdf (pip install pypdf) "
            "para descifrar el manual"
        )
        return []

    reader = PdfReader(str(pdf_path))
    if reader.is_encrypted:
        reader.decrypt("")
    written: list[Path] = []
    for page_number, page in enumerate(reader.pages, start=1):
        for image_number, image in enumerate(page.images, start=1):
            base = f"{pdf_path.stem}_page{page_number:02d}_image{image_number:02d}"
            try:
                pil_image = image.image
            except Exception:
                pil_image = None
            if pil_image is not None:
                dest = out_dir / f"{base}.png"
                pil_image.save(dest)
            elif image.data.startswith(JPEG_MAGIC):
                dest = out_dir / f"{base}.jpg"
                dest.write_bytes(image.data)
            else:
                dest = out_dir / f"{base}.bin"
                dest.write_bytes(image.data)
            written.append(dest)
    return written


def extract_text(pdf_path: Path, out_dir: Path) -> list[Path]:
    """Vuelca el texto de cada página a un .txt (útil para manuales no escaneados)."""
    try:
        from pypdf import PdfReader
    except ImportError:
        return []

    reader = PdfReader(str(pdf_path))
    if reader.is_encrypted:
        reader.decrypt("")
    text_dir = out_dir / "text"
    text_dir.mkdir(parents=True, exist_ok=True)
    written: list[Path] = []
    for page_number, page in enumerate(reader.pages, start=1):
        text = page.extract_text() or ""
        if not text.strip():
            continue
        dest = text_dir / f"{pdf_path.stem}_page{page_number:02d}.txt"
        dest.write_text(text, encoding="utf-8")
        written.append(dest)
    return written


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__)
        return 2
    ref_dir = Path(argv[1])
    out_dir = Path(argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)

    total = 0
    for pdf_path in sorted(ref_dir.glob("*.pdf")):
        written = extract_with_pypdf(pdf_path, out_dir)
        if not written:
            written = extract_inline_images(pdf_path, out_dir)
        for path in written:
            print(f"imagen: {path}")
        for path in extract_text(pdf_path, out_dir):
            print(f"texto:  {path}")
        total += len(written)
    print(f"total: {total} imágenes y {sum(1 for _ in (out_dir / 'text').glob('*.txt'))} textos")
    return 0 if total or (out_dir / "text").exists() else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
