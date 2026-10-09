# Updates

Mochi looks for a new release on GitHub at start and every 12 hours. When there's one, the island says so once for that version, with View, which opens the Updates page in the settings, and Skip. The page says whether this Mochi is the latest, how it updates, and what each newer release brings, from its changelog. Check now looks again.

How it updates depends on how Mochi was installed, which the page finds by itself:

| Installed with | What the page offers |
|---|---|
| The install script | Update runs the script again, shows what it said if it fails, and restarts Mochi into the new version. Under systemd, the service restarts. |
| pacman, apt or dnf | The command for the package, like `paru -Syu mochi-bin`, with Copy and "Run in a terminal". |
| Nix | `nix flake update mochi` and a rebuild, with Copy. How you rebuild is yours: set `command`, like `["nh", "os", "switch", "--update"]`, and "Run in a terminal" runs it. |
| A debug build from the source tree | `git pull` and `cargo build` in the tree. |

"Run in a terminal" opens `terminal`, or xdg-terminal-exec, or `$TERMINAL`, and the terminal waits for Enter once the command ends, so what it said stays readable.

The check asks GitHub's API for the list of releases and sends nothing else. `check = false` turns it off; Check now still works. `mochi ipc updater check` looks right away.

| Action | What it does |
|---|---|
| `check` | Looks for a new release now |
| `view`, `skip` | Open the Updates page, or close the notice |
| `update` | Updates with the install script and restarts, for a Mochi it installed |
| `run`, `copy` | Run the update command in a terminal, or copy it |
| `open-release <version>` | Opens a newer release's page on GitHub |

```toml
{{#include ../../../../modules/updater/settings.toml}}
```
