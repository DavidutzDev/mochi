# Weather

The weather where you are, from [Open-Meteo](https://open-meteo.com), which needs no account or key: the sky and the temperature now, the next 24 hours and the next 7 days. The [control center](control-center.md) has a card with the weather now, the place and a strip of the next hours, and the [desktop](widgets.md) a Weather widget in [four looks](#the-widgets-looks).

Nothing is sent until you set a place. Until then, the card and the widget say so, and their **Set a place** button opens the setting. A place Open-Meteo doesn't know gets **Change the place**, and a fetch that failed **Try again**. Set `place` to a city or a town, like `"Lyon"`, or `"Lyon, France"` or `"Springfield, Illinois"` for the one in that country or region. Mochi looks the name up once with Open-Meteo's geocoding, and keeps where it is. Set `latitude` and `longitude` instead to skip the lookup: they count once both are set, and `place` then only names them on the card.

The forecast comes again every `refresh_minutes`, 30 by default. When a fetch fails, as before the network is up, the next try waits a minute, then two, then four, up to `refresh_minutes`; a place Open-Meteo doesn't know waits for you to change it. `$XDG_STATE_HOME/mochi/weather.json` keeps where the place is and the last forecast, so after a restart the weather shows at once and comes again only when it's due. A forecast older than six hours isn't shown. Changing the settings applies at once, without a restart.

Open-Meteo gets the place's name when it's looked up, then its coordinates with each fetch, and nothing else. A [bento](../bento.md) you share leaves `place`, `latitude` and `longitude` out.

| Action | What it does |
|---|---|
| `refresh` | Fetches the forecast now |
| `status` | Prints the weather now and today's low and high, like `Lyon: 15°C, overcast, feels like 13°C; today 10°C to 16°C. Updated 9 minutes ago` |

## The widget's looks

The drawer has the widget under Weather in four looks, and its settings switch between them. `mochi ipc widgets add weather current:<look>` places one.

| Look | Size | What it shows |
|---|---|---|
| `current`, Now | 16 by 6 | The sky's icon, the temperature, the sky in words, and the place with today's low and high |
| `icon`, Icon | 16 by 8 | The sky's icon in a cookie in the accent color, with the temperature big beside it over the sky in words |
| `forecast`, Forecast | 20 by 14 | A row a day from today, as many as fit: the weekday, the sky, the chance of rain or snow from 20%, and the low and high on either side of a bar. The bars share the week's scale, so a warm day sits to the right; today's has a dot at the temperature now |
| `hours`, Hours | 24 by 10 | The next 12 hours as a curve, with each hour's temperature over it and its sky and hour under it. The first is now, with a dot on the curve. Where the columns get narrow, every other hour has its labels |

Sizes are in grid cells, and every look grows with the widget. Without a forecast, each look says why, with the same button as the card. When the last fetch failed, or the forecast is older than `stale_minutes`, an hour by default, each look adds how old it is, like "Updated 2 hours ago". It's never less than twice `refresh_minutes`, so a forecast waiting for its next fetch doesn't count.

## The weather in other views

The module's state has the whole forecast, so any view can show it: a plugin, a widget of your own, a clock or a lock screen. Views read it with `Daemon.state("weather")`.

| Field | |
|---|---|
| `configured` | Whether a place or coordinates are set |
| `loading` | Whether a fetch runs |
| `error` | Why the last fetch failed, or `null` |
| `next` | When the next fetch is due, in seconds since the epoch, or `null` while one runs, while nothing is set, and after a place Open-Meteo doesn't know |
| `units` | `metric` or `imperial` |
| `unit` | The units' signs: `temperature` (`°C` or `°F`) and `speed` (`km/h` or `mph`) |
| `place` | `name`, `region`, `country`, `latitude` and `longitude`, or `null` before the lookup |
| `updated` | When the forecast came, in seconds since the epoch, or `null` |
| `stale_after` | How old `updated` gets, in seconds, before the looks say how old it is: `stale_minutes`, or twice `refresh_minutes` when that's longer |
| `timezone`, `utc_offset` | The place's time zone, like `Europe/Paris`, and its offset from UTC in seconds |
| `current` | The weather now, or `null` before the first forecast: `temperature`, `feels_like`, `humidity` (%), `wind_speed`, `wind_direction` (degrees, where the wind comes from), `uv_index`, `time` |
| `hourly` | The next hours, from the one that runs, up to 24: `time` (when the hour starts, in seconds since the epoch), `hour` (0 to 23, at the place), `temperature`, `precipitation` (the chance of rain or snow, %) |
| `daily` | The next days, from today, up to 7: `time` (midnight at the place), `date` (like `2026-10-09`), `weekday` (0 is Sunday), `min`, `max`, `precipitation` (%), `uv_index`, `sunrise`, `sunset` (seconds since the epoch) |

Temperatures and speeds are in the units set. Each of `current`, the hours and the days also has the sky: `code`, the WMO weather code, `text`, like "Partly cloudy", `icon`, a Material Symbols name for `Symbol`, like `partly_cloudy_day` or `clear_night`, and `kind`, one of `clear`, `partly`, `cloudy`, `fog`, `drizzle`, `rain`, `sleet`, `snow`, `storm` or `unknown`. `current` and the hours have `day`, false at night, which the icons follow; the days' icons are the day's.

```toml
{{#include ../../../../modules/weather/settings.toml}}
```
