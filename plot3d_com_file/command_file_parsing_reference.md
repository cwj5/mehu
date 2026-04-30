# PLOT3D Command-File Parsing Reference (from commands.F)

This document is a practical parsing spec derived from the legacy parser in `commands.F`.
It is intended to answer: what tokens are valid, how abbreviations resolve, what qualifiers
exist, which qualifiers take values, what positional arguments are consumed, and where
legacy interactive questions appear.

## 1. Command-Line Model

- One logical command per line, except commands that intentionally consume continuation input (notably `CONTOURS/MANUAL`).
- Command name appears first.
- Optional qualifiers follow using slash syntax: `/QUAL`, `/QUAL=value`, or grouped slash qualifiers.
- Remaining tokens are positional arguments consumed by command-specific readers.
- Command files can include indirect command files via `@file` syntax (handled by parser I/O layer initialized by `INITIO`).

## 2. Matching and Abbreviations

Legacy matching uses prefix logic via `MATCH`:

- A token is accepted when it is a unique prefix of exactly one candidate.
- If no candidate matches: unrecognized.
- If multiple candidates match: ambiguous.

This applies to:

- Top-level commands (`P3DCOM` command list).
- Qualifier keywords (`QUALIF` with grouped `QLIST` entries).
- Enumerated argument values read as text (for example colors, view names, YES/NO, AUTO).

Examples seen in source and existing docs:

- `CONTOURS/M` -> `MANUAL`.
- `CONTOURS/INC` -> `INCREMENT`.
- `CONTOURS/NOA` -> `NOATTRIBUTES`.

## 3. Top-Level Command Set

`P3DCOM` defines 21 command tokens:

1. `CONTOURS`
2. `FUNCTION`
3. `MINMAX`
4. `QUIT`
5. `LIST`
6. `READ`
7. `SUBSETS`
8. `WALLS`
9. `RAKES`
10. `VIEW`
11. `VPOINT`
12. `PLOT`
13. `HELP`
14. `SHOW`
15. `MAP`
16. `CLEAR`
17. `TEXT`
18. `VECTORS`
19. `EXIT` (same branch as `QUIT`)
20. `FSURFACE`
21. `AUTOMM`

## 4. Qualifier Group Semantics

Each command declares grouped qualifiers with:

- `NQSET`: number of qualifier groups.
- `NQLIST`: count of entries in each group.
- `QLIST`: concatenated qualifier names.

Interpretation:

- At most one choice per group.
- Omitted group means default behavior.
- Some qualifiers carry typed values (integer, real, string).
- Qualifier values are retrieved by qualifier-argument readers (`GETIQ`, `GETRQ`, `GETCQ`) using qualifier indices from `IALIST`.

## 5. Command Signatures

### 5.1 `CONTOURS`

Qualifier groups:

- Group 1: `AUTOMATIC | INCREMENT | MANUAL` (default: automatic)
- Group 2: `RANGE`
- Group 3: `LINEAR | CUBIC`
- Group 4: `ATTRIBUTES | NOATTRIBUTES`

Positional/continuation arguments:

- `AUTOMATIC`: integer contour count; `0` switches to manual entry.
- `INCREMENT`: real increment; `0` switches to manual entry.
- `MANUAL`: repeated blocks of `START [END INC]`.

Interactive question flow (legacy):

- May ask contour attributes per block (type, color sequence, line/symbol/shading, plane orientations).
- `MANUAL` mode may keep prompting for additional blocks until termination.

Parser implication:

- After `CONTOURS/MANUAL`, subsequent numeric and attribute-answer lines are continuation input, not necessarily new commands.

### 5.2 `FUNCTION`

- No qualifiers.
- Positional: function number (integer).

### 5.3 `MINMAX`

Qualifier groups:

- `X | NOX`
- `Y | NOY`
- `Z | NOZ`
- `INCREM`
- `XSCALE`
- `YSCALE`
- `ZSCALE`

Positional formats:

- If all axes together: `XMIN XMAX [XINC] YMIN YMAX [YINC] [ZMIN ZMAX [ZINC]]`.
- Axis-specific prompting also supported (`X`, `Y`, `Z` sections independently).

Qualifier values:

- `/XSCALE=<real>`, `/YSCALE=<real>`, `/ZSCALE=<real>`.

### 5.4 `QUIT` and `EXIT`

- Qualifier group: `SAVE`.
- No positional arguments.
- `EXIT` dispatches to the same implementation as `QUIT`.

### 5.5 `LIST`

Qualifier groups:

- `FORMATTED | UNFORMATTED | BINARY | TEXT | IEEE_DP`
- `OUTPUT`
- `CGNS`

Qualifier values:

- `/OUTPUT=<filename>`.

Positional/interactive selection:

- Chooses list target from `XYZ | Q | FUNCTION | CGNS`.

### 5.6 `READ`

Qualifier groups:

- `1D | 2D | 3D`
- `XYZ`
- `Q`
- `FUNCTION`
- `MDATASET`
- `MGRID`
- `FORMATTED | UNFORMATTED | BINARY | IEEE_DP`
- `PLANES | WHOLE`
- `JACOBIAN | NOJACOBIAN`
- `BLANK | NOBLANK`
- `CHECK | NOCHECK`
- `CGNS`

Qualifier values:

- `/XYZ=<file>`, `/Q=<file>`, `/FUNCTION=<file>`, `/MDATASET=<int>`, `/CGNS=<file>`.

Defaults/behavior:

- If none of `/XYZ`, `/Q`, `/FUNCTION`, `/CGNS` are explicitly provided, legacy defaults attempt at least XYZ and Q reads.

### 5.7 `SUBSETS`

Qualifier groups:

- `GRID`
- `ADD`
- `ATTRIBUTES | NOATTRIBUTES`
- `ALL`
- `NONE`

Qualifier values:

- `/GRID=<int>`.

### 5.8 `WALLS`

Qualifier groups:

- `GRID`
- `ADD`
- `ATTRIBUTES | NOATTRIBUTES`
- `ALL`
- `NONE`

Qualifier values:

- `/GRID=<int>`.

### 5.9 `RAKES`

Qualifier groups:

- `IJK | XYZ`
- `ADD`
- `ATTRIBUTES | NOATTRIBUTES`
- `READ | WRITE`
- `+TIME | -TIME | +-TIME`
- `MAXPOINTS`
- `SCALAR_FUNCTION | NOSCALAR_FUNCTION`

Qualifier values:

- `/READ=<file>`, `/WRITE=<file>`, `/MAXPOINTS=<int>`, `/SCALAR_FUNCTION=<int>`.

### 5.10 `VIEW`

- No qualifiers.
- Positional: one token from `XY XZ YZ YX ZX ZY TOP SIDE FRONT`.
- Prefix matching applies to these view names.

### 5.11 `VPOINT`

Qualifier group:

- `XYZ | ANGLES`

Positional:

- Exactly three numbers.
- `/XYZ`: `X Y Z`.
- `/ANGLES`: `PHI THETA RADIUS`.

### 5.12 `PLOT`

Qualifier groups:

- `OpenGL`
- `2D | 3D`
- `FULLSCREEN | NOFULLSCREEN`
- `LABELS | NOLABELS`
- `IJK | XYZ`
- `SURFACE | CARPET | LINE | CONTOUR | FSURFACE`
- `SCRIPT | NOSCRIPT`
- `AXES | NOAXES`
- `FIGURE | NOFIGURE`
- `BACKGROUND`
- `UP`
- `TITLE | NOTITLE`
- `BAR | NOBAR`
- `ADDITIONAL_TEXT | NOADDITIONAL_TEXT`
- `OVERLAY`

Qualifier values:

- `/FIGURE=<AREAX,AREAY,CHARHT>` (three real values)
- `/BACKGROUND=<color>` (`RGB` triggers additional R/G/B reals)
- `/UP=<axis>` where axis is from `X Y Z +X +Y +Z -X -Y -Z`
- `/OVERLAY=<window-int>`

### 5.13 `HELP`

- No qualifier parsing in `HLPCMD`; command remainder is passed through as help keywords.

### 5.14 `SHOW`

- No qualifiers.
- Positional subcommand matched by prefix among:
  `CONTOUR FUNCTION MINMAX SUBSETS WALLS RAKES VIEW VPOINT VECTOR FSURFACE PLOT TEXT`.

### 5.15 `MAP`

- No qualifiers, no positional args.

### 5.16 `CLEAR`

- No qualifiers, no positional args.

### 5.17 `TEXT`

- No qualifiers.
- Consumes up to two text lines via interactive text-entry routine.

### 5.18 `VECTORS`

Qualifier groups:

- `SCALAR_FUNCTION | NOSCALAR_FUNCTION`
- `LENGTH_SCALE`
- `ATTRIBUTES | NOATTRIBUTES`

Qualifier values:

- `/SCALAR_FUNCTION=<int>`
- `/LENGTH_SCALE=<real>`

### 5.19 `FSURFACE`

Qualifier groups:

- `SCALE_FACTOR`
- `WALLS_ORIGIN`
- `CONTOUR | GRID`

Qualifier values:

- `/SCALE_FACTOR=<real|AUTO>`
- `/WALLS_ORIGIN=<real|AUTO>`

### 5.20 `AUTOMM`

Qualifier groups:

- `GRID`

Qualifier values:

- `/GRID=<int>`

## 6. Common Typed Inputs and Question Patterns

Common reader patterns in command handlers:

- Integer: `GETIA`, `GETIQ`.
- Real: `GETRA`, `GETRQ`.
- Character/text: `GETCA`, `GETCQ`.
- Free argument token parsing: `REAARG`, `RDARG`, `ENDARG`.

Common enumerated text question families:

- Color names (with special `RGB` branch requiring three numeric components).
- YES/NO style toggles.
- View names.
- Axis names for `/UP`.
- `AUTO` vs numeric values (notably FSURFACE settings).

## 7. Practical Rules for a Deterministic Script Parser

1. Use case-insensitive unique-prefix matching for commands, qualifiers, and enum-valued textual arguments.
2. Model qualifiers as grouped choices (one active value per group).
3. Support qualifier values and positional values together on one line.
4. Enforce required value counts for commands like `VPOINT` and `MINMAX` forms.
5. Treat continuation-answer lines after interactive-style commands as command-owned payload, especially `CONTOURS/MANUAL`.
6. Emit explicit diagnostics for unknown or ambiguous tokens instead of silently guessing.

## 8. Source Provenance

Primary source: `plot3d_com_file/commands.F`.

Key sections:

- Top-level command dispatch: `P3DCOM`.
- Qualifier definitions: each `*CMD` routine (`CONCMD`, `PLTCMD`, `REACMD`, etc.).
- Positional/question flows: corresponding `*CM1` routines and helper routines (`CONAUT`, `CONINC`, `CONMAN`, `VPTXYZ`, `VPTANG`, `MINALL`, `MINSET`, `SHOCM1`).