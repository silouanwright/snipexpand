# YAML editor support

SnipExpand provides separate JSON schemas for settings (`config.schema.json`)
and match files (`match.schema.json`). They describe SnipExpand's supported
format, including date timezones. They are not substitutes for Espanso's schemas.

## Export the schemas from an installed binary

```bash
mkdir -p ~/.config/snipexpand/schemas
snipexpand schema config > ~/.config/snipexpand/schemas/config.schema.json
snipexpand schema match > ~/.config/snipexpand/schemas/match.schema.json
```

If you use `XDG_CONFIG_HOME`, substitute that directory for `~/.config`.
Export again after upgrading to keep your editor's suggestions current.
Exporting prints JSON to stdout without creating configuration or contacting
the daemon; the shell redirection saves the files.

## Associate a YAML file

In an editor using YAML Language Server (such as VS Code with the Red Hat YAML
extension, or Neovim configured with `yaml-language-server`), add a comment at
the top of the YAML file. For `config.yml`:

```yaml
# yaml-language-server: $schema=./schemas/config.schema.json
```

For a file directly inside `match/`:

```yaml
# yaml-language-server: $schema=../schemas/match.schema.json
```

For deeper match subdirectories, adjust the relative path to the exported
schema. Paths are relative to the YAML file. These comments do not affect
SnipExpand parsing. Existing editor language-server setup is required;
SnipExpand does not install editor plugins.

The source versions in this directory can also be associated directly using
absolute paths. The schemas are embedded in release binaries and included in
the Cargo package, so schema export works offline.

## What the editor checks

- Supported keys, value types, trigger alternatives, and numeric settings ranges.
- Date and nested-match parameter shapes, including the `tz` field.
- Required profile filters and case-propagation settings.

Run `snipexpand check` for semantic validation: valid date formats and ranges,
known timezone names, variable names, regex syntax, duplicate variable names,
profile paths, and nested-reference cycles or missing targets. The editor does
not have the complete loaded configuration or the runtime timezone database.
Schema success alone does not prove a snippet can expand in an application.

`cargo test --test authoring_cli` validates representative YAML and invalid
configuration cases against both the schemas and the CLI parser. Cases requiring
semantic checks explicitly expect the editor schema to accept them and `check`
to reject them. The schema validator is a development-only dependency with
network and file-reference resolution disabled.
