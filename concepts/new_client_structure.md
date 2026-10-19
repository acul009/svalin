> Written with Codex.

# New Client Structure

Store client state by data type first, then by device:

- **Persistent:** names, groups, reports; saved and synchronized.
- **Live:** CPU usage, memory, processes; populated while one or more paths access it, deleted when the last path releases it.
- **Server:** connection status and other server-supplied data.

Each category owns its storage and update rules.

Expose `state.device(&spki_hash)` as a borrowed view combining references to a device's data. Live and server data are optional. Nothing is copied or duplicated.

## Navigation

Use a nested `Path` enum chain to describe a destination, such as Devices → Device → Reports or Server → Sessions.

Each tab holds a path. Tabs can detach into separate windows, which hold the same path. Navigation paths replace the destination-selection parts of the current `Screen` logic; shared client data stays independent of tabs and windows.

Active paths determine which live data is collected. Tabs and windows accessing the same data share it; collection stops and the data is removed when no path needs it anymore.
