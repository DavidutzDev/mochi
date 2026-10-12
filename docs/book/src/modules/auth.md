# Auth

Passwords and what the session remembers of them. polkit's admin passwords, the keyring's, and those `sudo -A` and ssh ask for are typed on the island, and locking the screen forgets sudo's password, the SSH keys in the agent, the admin actions polkit remembers and the keyring's unlocked secrets.

It's off until you add `"auth"` to `modules`. It takes over password prompts for the whole session, so it waits until you choose it.
## Admin passwords

When an app needs an admin action, like `pkexec`, `systemctl restart`, `run0` or a settings app, polkit asks Mochi, and the island shows a prompt: which program asks, polkit's reason, and for `pkexec` the command it would run. When several users may allow it, a choice picks who types. Enter checks the password, Escape or Cancel declines, and three wrong tries end it. A wrong one clears the field and says so.

Mochi never checks the password itself: polkit's own helper does, with PAM, and tells polkit. The password goes from the island to the auth module and on to the helper, and isn't kept or logged.

Only one polkit agent answers for a session. If another runs, like hyprpolkitagent, Mochi logs that it can't answer and the other one keeps asking. Turn it off: with the home-manager module, `programs.mochi.polkitAgent.enable` turns off `services.hyprpolkitagent`; otherwise stop its service or remove it from the compositor's autostart. `polkit = false` in `[module.auth]` leaves polkit to another agent.

## The keyring

gnome-keyring asks for passwords through a prompter on the session bus, GNOME's `gcr-prompter` by default. Mochi takes its place, so the island asks: to unlock a keyring, for a new keyring's password, typed twice, and for the passphrases gcr's SSH agent needs. The prompt shows what the keyring says, with its choice to tick when it has one, like unlocking the keyring at login. A wrong password brings the keyring's own warning. The password goes back to gnome-keyring encrypted, with the key exchange gcr uses, so it isn't readable on the bus.

While gcr-prompter runs, it keeps the prompts: it exits after a few idle seconds, and Mochi takes them from then on. `keyring = false` in `[module.auth]` leaves them to gcr-prompter.

## sudo and ssh

`sudo -A`, and ssh when it has no terminal, run the program in `SUDO_ASKPASS` or `SSH_ASKPASS` to ask for a password. Point both at `mochi-askpass`, which comes with Mochi, and they ask on the island: what the program asks, in its words, with Continue and Cancel. Cancel makes sudo or ssh stop. The home-manager module's `askpass.enable` sets both.

- Plain `sudo` keeps asking in its terminal. `alias sudo='sudo -A'` sends it to the island; `run0` asks through polkit already.
- ssh asks on the island from a terminal too with `SSH_ASKPASS_REQUIRE=prefer`.
- For an SSH key's passphrase, the prompt has a choice of how long the key stays in the agent once added: until the screen locks, 1, 4 or 8 hours, or until logout, starting from `remember_ssh_keys`. ssh adds the key with `AddKeysToAgent yes` in its config, or `ssh-add` does.
- ssh's yes or no questions, like confirming each use of a key, have Yes and No. Its notices, like touching a security key, show for a while and need no answer.

The password goes from the island to the auth module, back to `mochi-askpass` and to the program that ran it. Any program of the user's could run `mochi-askpass` too, so the prompt says which program asks: the one that ran it, as the kernel says, with its command line. Before it shows the prompt without a warning, Mochi checks that the answer goes only to that program: `mochi-askpass` answers on a pipe, and the program named is the only one Mochi sees reading it. A program running as root also has to be sudo started with `-A`, with nothing else started. That's how sudo shows as running as root, which no other program of the user's can show. It checks again before the answer goes out, and if anything changed, it keeps the answer and says why. A red line under the prompt says to look twice when the program runs from a folder you can write to, asked Mochi without `mochi-askpass`, exited before the prompt showed, or doesn't pass the check. The command line is the program's own to set, so trust the name above it more.

Only the island answers a prompt. `mochi ipc auth answer` from anything else, even a program that connects as the UI, is refused, so no program can say Yes to ssh's questions for you. mochid knows who wrote each message from the kernel, for every message, so a program can't pass a connection to another or become `mochi` after connecting to look like it. If the UI isn't the Quickshell mochid started, like one started by a `--quickshell` script that doesn't `exec` it, the island can't answer prompts at all; mochid logs it, and `mochi doctor` says so.

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

## What it can't stop

The prompts protect against programs that aren't yours. A program you run, with your user's rights, has more ways in:

- It can run `mochi-askpass` itself and gets the password you type. The prompt names it, from the file it runs, and the red line warns when something looks off. Read the name before typing: sudo asks as root, and ssh and ssh-add ask for a key's passphrase.
- A program can hide its open files from Mochi (`PR_SET_DUMPABLE`), as ssh-agent does. One like that could read the pipe the answer goes on without Mochi seeing it. It has to run as you already. A prompt you didn't start is the sign to look for.
- It can ask for a keyring password over the session bus, as gnome-keyring does, and gets what you type. gcr-prompter has the same gap. Mochi's keyring prompts don't say who asks yet.
- The password passes through mochid and Quickshell. Mochi's Rust code clears its own copies once they're used, but socket buffers and Quickshell's JavaScript keep theirs until that memory is reused. Only a program running as you can read them there.
- Plugins' views run inside the shell, beside the island's own, so a plugin can answer a prompt as the island does. Install plugins you trust.
- `mochi-askpass` counts as Mochi's only beside the mochid that runs. After an update, until mochid restarts, every prompt warns that the program asked directly. home-manager restarts it on switch.

No other program can answer a prompt for you: only the island does.
