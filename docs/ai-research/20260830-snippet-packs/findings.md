# Findings: snippet packs

## Research question

Should SnipExpand support Git-published snippet packs, and what is the smallest
safe product model worth building?

## Conclusion

A pack should initially be an ordinary Git repository or repository directory
containing declarative YAML and lightweight metadata. SnipExpand should install
it locally, pin its revision, validate it, and support explicit updates and
removal. A central registry, accounts, executable hooks, dependency solving,
and automatic background updates are not justified initially.

## What exists

- Espanso offers the closest direct precedent: a package is normal match YAML,
  a manifest, and a README. Its CLI installs, lists, updates, and removes Hub or
  external Git packages. The Hub adds discovery and review.
- TextExpander models reusable collections as public groups. Subscribers receive
  author updates, while the subscribed group stays read-only; customization
  starts by duplicating it.
- Alfred uses portable snippet collections without a registry. Raycast combines
  file import with searchable tags and managed team collections.
- VS Code distributes declarative snippet bundles through its extension
  marketplace. Homebrew taps demonstrate that independent Git repositories can
  remain the source of truth while a CLI provides a coherent install experience.

## Recommended SnipExpand model

### Author experience

A repository may contain one pack at its root or several packs in named
subdirectories. A pack contains:

- `pack.yml`: name, title, version, description, author, license, tags, and
  optional homepage
- `match/**/*.yml`: ordinary SnipExpand match files
- `README.md`: optional longer documentation
- `LICENSE`: strongly recommended for public packs

Pack files remain fully declarative. They cannot contain install hooks, shell
commands, scripts, or dependencies.

### Existing Espanso packs

SnipExpand should also auto-detect Espanso repositories containing
`_manifest.yml` and `package.yml`. If the package uses only fields supported by
SnipExpand's strict compatibility layer, install it directly without copying or
rewriting the source. If it uses unsupported behavior, reject the pack with a
complete field-level compatibility report. Never install a partially working
pack.

This is narrower than personal Espanso configuration migration. Pack support
handles a versioned, self-contained repository; migration must reason about a
user's whole configuration and remains deferred until the intended feature
surface is complete.

### User experience

The intended command surface is:

```text
snipexpand pack install URL [--path DIR] [--ref TAG_OR_COMMIT]
snipexpand pack inspect URL [--path DIR] [--ref TAG_OR_COMMIT]
snipexpand pack list
snipexpand pack update [NAME|--all]
snipexpand pack remove NAME
```

Git is an implementation detail. Installation records the resolved commit,
validates the whole pack, reports trigger conflicts, and enables it as a named
group. Updates are explicit rather than automatic.

### Ownership and customization

Installed packs should be read-only from SnipExpand's perspective. This avoids
silently overwriting local edits during updates. A later `pack fork` or `copy`
command can copy a pack or selected snippets into the user's editable match
directory.

### Discovery

Do not build a Hub initially. A Git URL is enough to publish and install a pack.
Once several useful third-party packs exist, a curated Markdown or JSON index
can add discovery without changing the package transport. Search, ratings,
accounts, and automated publishing are premature.

## Build order

1. Named groups with enable and disable state.
2. Pack manifest and validation.
3. Install, inspect, list, and remove native and compatible Espanso packs from
   Git.
4. Explicit pinned updates.
5. Plugin UI for installed packs.
6. A curated index only after real packs exist.

Espanso migration remains deferred until SnipExpand's desired compatibility
surface is complete. Packs do not require Espanso import support.
