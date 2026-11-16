# Third-party software

- Electron and electron-builder are distributed under the MIT license. Electron includes Chromium and Node.js, with their respective third-party license notices in the packaged application.
- Tesseract.js and Tesseract.js Core are distributed under Apache-2.0. The application's OCR engine bundles their published JavaScript/WASM artifacts.
- FFmpeg and FFprobe are independently installed and are not redistributed here. Refer to the terms of the FFmpeg build you install.
- Optional online catalogue queries use MusicBrainz.
- HEIC conversion uses the ISC-licensed `heic-convert` and `heic-decode`, PNGJS (MIT), jpeg-js (BSD-3-Clause), and libheif-js 1.23.5 (LGPL-3.0). The libheif-js runtime is kept unpacked in the desktop distribution so it can be replaced. See [libheif-js v1.23.5 source and build scripts](https://github.com/catdad-experiments/libheif-js/tree/v1.23.5) and [upstream libheif source](https://github.com/strukturag/libheif). The LGPL notice is in `licenses/libheif-js.txt`.
- PDF extraction uses PDF.js (Apache-2.0) with its bundled character maps and standard fonts. See `licenses/pdfjs.txt`, `licenses/pdfjs-cmaps.txt`, `licenses/pdfjs-foxit-fonts.txt` and `licenses/pdfjs-liberation-fonts.txt`.
- DOCX extraction uses fflate (MIT). See `licenses/fflate.txt`.
