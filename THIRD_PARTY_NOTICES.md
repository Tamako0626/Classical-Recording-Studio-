# Third-party software

- The application is MIT. Its window toolkit is GPUI Kit (Apache-2.0): https://github.com/longbridge/gpui-kit
- Playback uses rodio. File dialogs use rfd. HTTP uses ureq. JSON uses serde.
- MusicBrainz is queried read-only. See https://musicbrainz.org/doc/MusicBrainz_API
- The bundled `ffmpeg` and `ffprobe` are separate GPL programs because they include libx264. The application only executes them. Their source tarballs, the build script `scripts/build_ffmpeg.sh`, and the license texts are in `ffmpeg-SOURCE.txt` and `ffmpeg-COPYING.txt` inside the release zip.
- LAME (used as libmp3lame) is LGPL. zlib is under the zlib license. Both are linked into the FFmpeg sidecar, not into the MIT application.
