# Source ledger: snippet packs

## Scope fence

Current lane: distribution, installation, updating, and trust models for
shareable text-expansion snippet packs.

Allowed roots:

- SnipExpand documentation and source
- Official documentation and repositories for text expanders, snippet tools,
  editor snippet ecosystems, and package managers

Forbidden roots:

- Unrelated SnipExpand feature research
- Local personal snippet contents

| Source | Date checked | Tier | Relevance |
| --- | --- | --- | --- |
| [Espanso package basics](https://espanso.org/docs/packages/basics/) | 2026-08-30 | 1 | Packages are YAML plus metadata with install, version selection, list, update, and uninstall commands |
| [Espanso external packages](https://espanso.org/docs/packages/external-packages/) | 2026-08-30 | 1 | Public and private Git repositories can serve packages without the Hub |
| [Espanso package creation](https://espanso.org/docs/packages/creating-a-package/) | 2026-08-30 | 1 | Uses a manifest, ordinary match YAML, and README; supports independent Git repositories |
| [Espanso package specification](https://espanso.org/docs/packages/package-specification/) | 2026-08-30 | 1 | Defines identity, SemVer, author, tags, homepage, license, and multiple match files |
| [Espanso Hub repository](https://github.com/espanso/hub) | 2026-08-30 | 1 | Central catalog uses CI plus human review because packages may contain executable behavior |
| [TextExpander public groups](https://textexpander.com/learn/using/public-groups/contributing-to-snippets-in-public-groups) | 2026-08-30 | 1 | Public groups are subscribable, author-controlled collections with metadata, licensing, review, and updates |
| [TextExpander group installation](https://textexpander.com/learn/using/snippet-groups/adding-snippet-groups-to-textexpander) | 2026-08-30 | 1 | Subscribed public groups remain read-only; users duplicate them to customize |
| [Alfred snippets](https://www.alfredapp.com/help/features/snippets/) | 2026-08-30 | 1 | Collections provide grouping and portable export without a package registry |
| [Raycast snippets](https://manual.raycast.com/snippets) | 2026-08-30 | 1 | Supports file import, tags, search, and centrally managed team snippets |
| [VS Code snippet guide](https://code.visualstudio.com/api/language-extensions/snippet-guide) | 2026-08-30 | 1 | Declarative snippet bundles can be published as extensions with manifest metadata |
| [Homebrew taps](https://docs.brew.sh/Taps) | 2026-08-30 | 1 | A mature Git-backed distribution model keeps third-party repositories independent and makes trust explicit |
