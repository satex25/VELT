# @velt/shell — STUB

Not implemented. The Electron host spawns the Rust daemon as a sidecar and
parses the `VELT_DAEMON_LISTENING 127.0.0.1:<port>` line from its stdout to
discover the port. Keep that line format stable; it is the contract.
