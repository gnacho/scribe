# Privacy

Scribe does not collect, transmit or store any data about you. It has no
telemetry, no analytics, no crash reporting, no accounts and no cloud sync.

- The only outbound requests are the ones your document asks for. When an image
  points at a URL, or a video card needs its thumbnail, Scribe downloads that
  file so it can paint it, and caches it under `~/.cache/scribe/images`. Delete
  that directory at any time and the files are fetched again when needed.
- Opening a document that references remote images makes those requests, so the
  server hosting the image sees them (your IP address and roughly when you
  opened the document). Nothing else about the document is sent: the text
  itself is never uploaded.
- If you would rather not, keep images local (`![alt](path)`) or work offline:
  without connectivity the pictures keep their placeholder and nothing is
  retried behind your back.
- Documents are plain Markdown files on your disk. Scribe reads and writes only
  the files you open or save.
- Preferences are stored through GSettings in your user profile. Templates are
  plain files under `~/.local/share/scribe/templates`.

Everything you write stays on your machine.
