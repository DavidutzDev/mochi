# Auth

How long the session remembers what lets someone at the computer in without a password: sudo's password, the SSH keys in the agent, the admin actions polkit remembers, and the keyring's unlocked secrets.

By default, all of them are forgotten once the screen is locked, and whoever unlocks types them again. Each can be remembered longer, with a setting in `[module.auth]`:

| Setting | Choices | Default |
|---|---|---|
| `remember_ssh_keys` | `until-locked`, `1h`, `4h`, `8h`, `until-logout` | `until-locked` |
| `remember_keyring` | `until-locked`, `1h`, `4h`, `8h`, `until-logout` | `until-locked` |
| `remember_sudo` | `until-locked`, `own-timeout` | `until-locked` |
| `remember_polkit` | `until-locked`, `own-timeout` | `until-locked` |

A time counts from when the key was added or the keyring unlocked, or from when Mochi started for one that was already there, and a lock doesn't cut it short. Mochi checks every minute and takes back what's been there longer: `ssh-add -d` for a key, however it was added, and the Secret Service's `Lock` for the keyring, gnome-keyring's or KeePassXC's. `until-logout` leaves them to the agent and the keyring. sudo and polkit never keep a password longer than their own timeout, 5 minutes unless their configuration says otherwise, so `own-timeout` only keeps it through a lock.

Taking things back never holds the lock back, and one that fails, or finds nothing, doesn't stop the others.

## When the screen locks

Locking happens in several ways, and the module hears of each:

- logind's Lock: `loginctl lock-session`, hypridle's usual `lock_cmd`, and the [power](power.md) module's Lock with no `lock` setting.
- The [lock](lock.md) module, once its lock is on screen.
- The power module's Lock with a `lock` command of its own, like `["hyprlock"]`, once it has started it.
- `mochi ipc auth locked`, for a keybinding that starts a locker directly: `hyprlock & mochi ipc auth locked`.

`mochi ipc auth forget` forgets everything at once, whatever the settings say.
