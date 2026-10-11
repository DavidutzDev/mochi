# Auth

Passwords and what the session remembers of them. polkit's admin passwords, the keyring's, and those `sudo -A` and ssh ask for are typed on the island, and locking the screen forgets sudo's password, the SSH keys in the agent, the admin actions polkit remembers and the keyring's unlocked secrets.

## Admin passwords

When an app needs an admin action, like `pkexec`, `systemctl restart`, `run0` or a settings app, polkit asks Mochi, and the island shows a prompt: which program asks, polkit's reason, and for `pkexec` the command it would run. When several users may allow it, a choice picks who types. Enter checks the password, Escape or Cancel declines, and three wrong tries end it. A wrong one clears the field and says so.

Mochi never checks the password itself: polkit's own helper does, with PAM, and tells polkit. The password goes from the island to the auth module and on to the helper, and isn't kept or logged.

Only one polkit agent answers for a session. If another runs, like hyprpolkitagent, Mochi logs that it can't answer and the other one keeps asking. Turn it off: with the home-manager module, `programs.mochi.polkitAgent.enable` (on by default) turns off `services.hyprpolkitagent`; otherwise stop its service or remove it from the compositor's autostart. `polkit = false` in `[module.auth]` leaves polkit to another agent.

## The keyring

gnome-keyring asks for passwords through a prompter on the session bus, GNOME's `gcr-prompter` by default. Mochi takes its place, so the island asks: to unlock a keyring, for a new keyring's password, typed twice, and for the passphrases gcr's SSH agent needs. The prompt shows what the keyring says, with its choice to tick when it has one, like unlocking the keyring at login. A wrong password brings the keyring's own warning. The password goes back to gnome-keyring encrypted, with the key exchange gcr uses, so it isn't readable on the bus.

While gcr-prompter runs, it keeps the prompts: it exits after a few idle seconds, and Mochi takes them from then on. `keyring = false` in `[module.auth]` leaves them to gcr-prompter.

## sudo and ssh

`sudo -A`, and ssh when it has no terminal, run the program in `SUDO_ASKPASS` or `SSH_ASKPASS` to ask for a password. Point both at `mochi-askpass`, which comes with Mochi, and they ask on the island: what the program asks, in its words, with Continue and Cancel. Cancel makes sudo or ssh stop. The home-manager module's `askpass.enable`, on by default, sets both.

- Plain `sudo` keeps asking in its terminal. `alias sudo='sudo -A'` sends it to the island; `run0` asks through polkit already.
- ssh asks on the island from a terminal too with `SSH_ASKPASS_REQUIRE=prefer`.
- For an SSH key's passphrase, the prompt has a choice of how long the key stays in the agent once added: until the screen locks, 1, 4 or 8 hours, or until logout, starting from `remember_ssh_keys`. ssh adds the key with `AddKeysToAgent yes` in its config, or `ssh-add` does.
- ssh's yes or no questions, like confirming each use of a key, have Yes and No. Its notices, like touching a security key, show for a while and need no answer.

The password goes from the island to the auth module, back to `mochi-askpass` and to the program that ran it. Any program of the user's could run `mochi-askpass` too, so the prompt says which program asks: the one that ran it.

## What it remembers

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
