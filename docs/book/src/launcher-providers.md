# Launcher providers

A plugin can add results to the [launcher](modules/launcher.md): an emoji picker, bookmarks, a password manager, anything you'd search for. The launcher asks the plugin's backend with each query and shows what it answers, under the plugin's heading. For results from a script, without a plugin, see [script providers](modules/launcher.md#script-providers).

The emoji picker in `examples/plugins/emoji` is a complete one: `;cat` lists cats, Enter copies the one you pick, and the ones you picked come first next time. Mochi's own emoji module is built the same way, with `:`.

## The contribution

A provider is a contribution to the launcher, without a view:

```toml
[[contributions]]
target = "launcher"
kind = "provider"
id = "emoji-example"
title = "Emoji"
options = { prefix = ";", search = "search", pick = "pick" }

[[actions]]
name = "search"
description = "Search emoji by name"
args = [{ name = "query", kind = "string", optional = true, rest = true }]

[[actions]]
name = "pick"
description = "Remember an emoji the user picked"
args = [{ name = "id" }]
```

| Option | | |
|---|---|---|
| `prefix` | | A query starting with it asks only this provider, with the rest. Without one, the provider is asked on every query that isn't empty, next to the apps. When prefixes overlap, the longest wins; one can end with a space, like the timer's `:t `, so `:tea` still goes to the emoji's `:` |
| `search` | `"search"` | The action that answers a query. It takes the query as one optional `rest` argument. |
| `pick` | | An action taking a result's `id`, called after the user picks a result that has one. |

`id` names the provider in the launcher's settings, so users can change its prefix, its `order` or its heading, or turn it off, with a `[module.launcher.providers.<id>]` section. The contribution's `order` is where it goes by default; apps are at 0.

## Answering

The launcher calls the `search` action as `mochi ipc` would, a moment after the user stops typing, and reads the answer: one result per line, as JSON. In the Rust SDK, answer with `command.answer`:

```rust
ModuleEvent::Command(command) if command.action == "search" => {
    let query = command.args.str("query").unwrap_or("");
    let lines: Vec<String> = find(query)
        .map(|emoji| {
            json!({
                "title": emoji.name,
                "subtitle": emoji.group,
                "glyph": emoji.glyph,
                "copy": emoji.glyph,
                "id": emoji.glyph,
            })
            .to_string()
        })
        .collect();
    command.answer(Ok(lines.join("\n")));
}
```

A result is the same as a [script provider's](modules/launcher.md#script-providers): a `title`, and optionally a `subtitle`, an `icon`, a `glyph` or a `color`, at most one of `copy`, `type`, `open` and `run` for what Enter does, an `alt` with what Shift+Enter does, and an `id`. A result with only an `id` leaves everything to `pick`, which can then do whatever the plugin wants. Answer with nothing for no results; an error is logged and shows no results.

The launcher asks at once, on every keystroke, so answer quickly. Answers to a query the user has typed past are dropped, so a slow provider never shows stale results. Keep the answer under 50 results; the launcher cuts the list at its `max_results`.

## Picking

When the user picks a result with an `id`, the launcher does the result's verb, closes, and calls the `pick` action with that `id`. The emoji picker uses it to put recently picked emoji first.

## Taking contributions yourself

The launcher learns about providers from `ModuleEvent::Offers`: every module gets the contributions whose `target` is its id, at the start and after each reload that changes them. A plugin can take contributions the same way, for things other plugins add to it.
