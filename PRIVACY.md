# Privacy

Scribe makes no network requests. It does not collect, transmit, or store any
data outside your machine.

- No telemetry, no analytics, no crash reporting, no accounts, no cloud sync.
- No network code is linked into the binary. Remote image URLs are detected
  only to be ignored, so a remote image is never fetched.
- Documents are plain Markdown files on your disk. Scribe reads and writes only
  the files you open or save.
- Preferences are stored through GSettings in your user profile. Templates are
  plain files under `~/.local/share/scribe/templates`.

Everything you write stays on your machine.
