# `imx pipeline`

`imx pipeline` applies an ordered list of geometry/resize operations in a
**single decode → transform → encode pass**. The existing per-operation
subcommands (`resize`, `resize-fit`, `crop`, `rotate`, `flip`, `flop`) each
re-decode and re-encode the image, so chaining them means N decodes and N
encodes. `pipeline` decodes the input once, applies every `--op` in order, and
encodes the result once.

## Synopsis

```
imx pipeline [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:-> --op <op> [--op <op> ...]
```

- Input and output use the same `[FORMAT:]<path>` machinery as every other
  command, including format prefixes (e.g. `PNG:in.png`) and stdin/stdout
  streaming via `-` with a required prefix on stdout (e.g. `PNG:-`).
- At least one `--op` is required. Ops apply left-to-right in the order given.

## Supported operations

| `--op` value | Effect | Underlying `imx_core::Image` method |
| --- | --- | --- |
| `resize:<geometry>` | Nearest-neighbor resize. `<geometry>` is the same grammar as `imx resize`: `WxH`, `Wx`, `xH`, or `N%`. | `ResizeGeometry::parse` → `resolve` → `resize_nearest` |
| `resize-fit:<width>x<height>` | Aspect-preserving fit into the box. | `resize_nearest_fit` |
| `crop:<width>x<height>+<x>+<y>` | Bounds-checked crop. | `crop` |
| `rotate:<90\|180\|270>` | Clockwise rotation. | `rotate_90` / `rotate_180` / `rotate_270` |
| `flip` | Vertical flip (no argument). | `flip_vertical` |
| `flop` | Horizontal flop (no argument). | `flop_horizontal` |

The op name is everything before the first `:`; the remainder is its argument.
The geometry, dimension, crop, and angle grammars are parsed by the exact same
helpers the standalone subcommands use, so a given op spec behaves identically
to the matching subcommand.

## Ordering semantics

Operations are applied strictly left-to-right, each consuming the output of the
previous one. **Order is significant.** For example:

```
imx pipeline in.png a.png --op rotate:90 --op crop:2x2+0+0   # rotate, then crop
imx pipeline in.png b.png --op crop:2x2+0+0 --op rotate:90   # crop, then rotate
```

produce different images: the crop operates on different pixels depending on
whether the rotation has already happened.

## Determinism and equivalence

The output is byte-deterministic: running the same pipeline on the same input
twice yields identical output bytes. The result is also byte-for-byte identical
to running the same operations as a chain of individual subcommands, e.g.

```
imx pipeline in.png out.png --op resize:50% --op rotate:90 --op flip
```

is equivalent to

```
imx resize 50%  in.png step1.png
imx rotate 90   step1.png step2.png
imx flip        step2.png out.png
```

The only difference is that `pipeline` performs a single decode and a single
encode instead of three of each. Both the determinism and the
subcommand-equivalence properties are covered by integration tests in
`crates/cli/tests/cli.rs`.

## Error handling

- An invalid op spec, an unknown op name, a missing op argument, or a bad
  geometry/angle is a **usage error** (exit code 2), reported through the same
  `fail_usage` path as the standalone subcommands.
- An op that parses but fails at runtime (e.g. a crop rectangle that extends
  past the image bounds) is an **operational error** (exit code 1). No output is
  written when any op fails.
