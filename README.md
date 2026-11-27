# SigmaOS System Profile

Declarative whole-system configuration — a SigmaOS config system `system.toml`
that composes every component's config into one system definition.

## Usage

```sh
sigmaos-system-profile --profile system.toml           # show resolved view
sigmaos-system-profile --profile system.toml --json    # JSON output
sigmaos-system-profile --profile system.toml --validate
sigmaos-system-profile --profile system.toml --list    # enabled components
sigmaos-system-profile --init                        # write default profile
```

## Profile format

```toml
name = "sigmaos-desktop"
version = "0.1.0"

[globals]
locale = "en_US.UTF-8"

[components.shell]
enabled = true
config = "~/.config/sigmaos/shell.toml"

[components.shell.options]
position = "top"
```

## Interconnectivity

- `sigmaos-config-system` — base config primitives
- `sigmaos-shell-config` — one of the composed components
- `sigmaos-update-system` — profile versioning aligns with rolling releases
