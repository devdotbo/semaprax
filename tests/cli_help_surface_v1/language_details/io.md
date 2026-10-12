### Network and filesystem details

- TCP: `net_connect`, `net_send`, `net_recv`, `net_stream_stdout`, `net_wait`,
  `net_close` need `permit`/`uses` of `network.connect`, `network.read`,
  `network.write` and an injected provider. Owned `net_recv` is refused inside
  `while` (`SPX-T270`). [Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md).
- `file_read` / `file_write_new` require explicit `fs.read` / `fs.write` and an
  injected [Filesystem I/O v1](FILESYSTEM-IO-V1.md) provider. Paths are bounded
  relative byte prefixes; writes create new files without overwriting.
  [Filesystem I/O v2](FILESYSTEM-IO-V2.md) adds stat, sorted immediate-name listing,
  mkdir, removal and atomic replacement. Only stat/list accept empty paths for
  the injected root. `std.fs` composes typed Path, FileInfo and Reader/Writer.
