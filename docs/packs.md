# Snippet packs

SnipExpand packs are read-only snippet collections installed from Git. Each
installed pack records its source, selected ref, resolved commit, metadata, and
enabled state. Updates happen only when requested.

## Native format

A native pack keeps its manifest at the repository root and ordinary match
files below `match/`:

```text
pack.yml
README.md
LICENSE
match/
  symbols.yml
```

The manifest uses this format:

```yaml
name: useful-symbols
title: Useful Symbols
version: 0.1.0
description: Common symbols and punctuation
author: Example Author
license: GPL-3.0-or-later
tags: [symbols, writing]
homepage: https://example.com/useful-symbols
```

`name`, `title`, `version`, `description`, and `author` are required. Pack names
may contain lowercase ASCII letters, numbers, and internal hyphens. Match files
use the same strict YAML format as personal snippets.

## Espanso format

SnipExpand auto-detects an Espanso pack when the selected directory contains
both `_manifest.yml` and `package.yml`. The package installs only when every
manifest and match field is supported. Scripts, forms, shell variables, imports,
and other unsupported behavior produce a validation error. Nothing is silently
omitted.

Use the official Hub shorthand to inspect or install the latest stable version
of a package:

```bash
snipexpand pack inspect espanso:arrows
snipexpand pack install espanso:arrows
```

An explicit update checks the Hub again and moves to its newest stable version.
The normal Git URL and `--path` form remains available for other repositories.

### Tested Hub compatibility

The following representative packages passed strict validation against Espanso
Hub commit `bcacd1af1f009e3463ea7863a493e1b48f11587d` from 2026-08-25:

- arrows, tableflip-package, quotes, supersubscript, spanish-accent,
  portuguese-accents, and numeronyms
- vim-digraphs, common-web-chars, and lean-symbols, including packages with
  thousands of matches or multiple YAML files

Packages using scripts, shell commands, clipboard variables, forms, or choices
were rejected as intended. Two sampled packages were also rejected for missing
nested trigger references. Validation proves that the package parses and uses
supported behavior; it does not prove every expansion through every app.

## Commands

```bash
snipexpand pack inspect GIT_URL
snipexpand pack install GIT_URL
snipexpand pack list
snipexpand pack update PACK
snipexpand pack update --all
snipexpand pack disable PACK
snipexpand pack enable PACK
snipexpand pack remove PACK
```

Use `--path DIR` with `inspect` or `install` when the pack is below the
repository root. Use `--ref REF` to select a branch, tag, or commit. SnipExpand
records the exact commit resolved during installation.

Install with `--disabled` to validate and retain a pack without loading its
snippets.

## Storage and safety

Repositories and state live below `$XDG_DATA_HOME/snipexpand/packs`, or
`~/.local/share/snipexpand/packs` when `XDG_DATA_HOME` is unset. Enabled match
files are mirrored below `~/.config/snipexpand/match/packs` so they use the same
loader and automatic reload path as personal snippets.

Pack repositories cannot contain symbolic links. Git hooks are disabled during
checkout, URLs containing embedded credentials are rejected, and installed
files are validated with the same bounded YAML parser as personal configuration.
Edit personal match files instead of the generated pack mirror. Disable or
remove a pack through the CLI.
