# Device protocols

All queries are read-only. Bytes are hexadecimal. Sources: [CREDITS.md](../CREDITS.md).

## WLmouse Beast G

USB `36A7:A860` (2.4 GHz receiver) or `36A7:A861` (mouse on its cable). HID usage page `FFFF`, usage `0000`, feature report 0, 65 bytes.

```text
set feature  00 | 00 00 02 02 00 83 00 …
get feature  00 | ST 00 02 02 00 83 CC NN …
```

| Field | Meaning |
|---|---|
| `ST` | `A1` answer ready, `A0` still waiting for the mouse |
| `CC` | `01` charging |
| `NN` | Battery percent |

Verified on hardware:

- While the mouse sleeps the receiver keeps answering `A0`; the query does not wake the mouse.
- With the cable attached the mouse appears as `A861` and answers with `CC = 01`, while the receiver answers `A0`.
- The collection with usage `0001` on the same page rejects the request.

## HyperX Cloud III S Wireless

USB `03F0:06BE` (`03F0:02CC` is untested). HID usage page `01C0`, usage `0001`, output and input report `0C`, 64 bytes.

```text
battery   out  0C 02 03 01 00 06 | 00 …
          in   0C 02 03 01 00 06 NN VH VL …
charging  out  0C 02 03 01 00 48 | 00 …
          in   0C 02 03 01 00 48 SS …
events    in   0D 02 03 00 0A xx   charging cable
               0D 02 03 00 0C xx   headset power
```

| Field | Meaning |
|---|---|
| `NN` | Battery percent; above 100 means the headset is off |
| `VH VL` | Voltage in millivolts, big-endian |
| `SS` | `00` not charging, `01` charging, `02` full |

Verified on hardware: battery, voltage and the change of `SS` from `00` to `01`. The headset being switched off and the value `02` come from the sources and are untested.
