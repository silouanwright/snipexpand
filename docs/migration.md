# Migrating from legacy TOML

Legacy `~/.config/snipexpand/expansions.toml` continues to load. YAML match files can
be introduced gradually without deleting or rewriting the TOML file.

Legacy:

```toml
[settings]
trigger_mode = "space"

[expansions]
";mail" = "user@example.com"
```

Equivalent YAML:

```yaml
# ~/.config/snipexpand/config.yml
trigger_mode: space
terminators: [space]
```

```yaml
# ~/.config/snipexpand/match/personal.yml
matches:
  - trigger: ";mail"
    replace: "user@example.com"
```

Run `snipexpand check` before removing `expansions.toml`. Duplicate triggers across
legacy TOML and YAML are rejected, preventing an ambiguous migration.

`snipexpand add` writes only to `match/generated.yml`; it never reformats manually
authored match files.
