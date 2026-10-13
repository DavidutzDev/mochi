# Profiling

When Mochi feels slow, uses a lot of CPU, or a game stutters while it runs, Mochi can say what it costs, and where.

## A report to send

```sh
mochi profile        # 30 seconds
mochi profile 60     # up to 300
```

While it records, use the computer as when the problem shows: play the game, play music, open what's slow. Then it prints the path of a report, one HTML file in `~/.local/state/mochi/profiles`. Open it in a browser, or attach it to a bug report. It works in every build, the released ones too.

The report has:

- **CPU** over time, every half second, for mochid, Quickshell and the programs they start, like `nvidia-smi` or `curl`, in percent of one core: 100% is one core busy.
- **Frames drawn**: how many frames each of the shell's windows drew each second, the island and the desktop of each monitor. Idle, it's close to 0. Something that moves draws up to the monitor's refresh rate, 144 times a second on a 144 Hz monitor, which is what costs on a gaming setup.
- **Wake-ups**: how often each process woke up. Many wake-ups with little CPU still keep a core from resting.
- **mochid's flame graph**: where mochid's own time went, function by function. Click a box to zoom in.
- The busiest threads, each module's time in its own task, memory, and the machine: Mochi's version, the compositor, the monitors, the CPU and the kernel.

Quickshell's QML has no flame graph: that needs a Quickshell built with Qt's QML profiler. Its threads, frames and wake-ups usually tell enough: a window drawing every frame while nothing should move points at an animation.

## Live, in dev mode

`mochid --dev`, which `cargo run -p mochid` starts, shows a bubble with its revision. Click it for the profiler: what mochid, Quickshell and the helpers cost now, the last minute of CPU, the frames each window draws, and the busiest threads and modules, twice a second. Record 30 s writes the same report as `mochi profile`, and Open the report opens it. Close, or a click on the bubble, stops it.

## What it costs

Nothing until something asks. While the panel is open or a recording runs, a thread reads the process tree from `/proc` twice a second, the shell counts the frames each window draws, and a recording samples mochid's stacks 199 times a second for the flame graph. Each module's time in its own task is counted all the time, which takes two clock reads each time the task runs. The panel and the report show the profiler's own thread, `mochi-profile`, with the rest.

A recording leaves about 60 MB in mochid until it restarts: the stack sampler keeps a table of samples ready for the next one, whatever its size. Naming the functions for the flame graph takes more for a moment, which mochid hands back once the report is written.

The flame graph needs mochid's function names. The release archives and the Nix and Arch packages keep them, and leave out the rest of the debug information.
