# Performance

CPU, memory and GPU use, the CPU's and GPU's temperatures, and disk and network speeds.

The control center has a **Performance page**: CPU, memory and GPU, each with its value, a detail line (threads and temperature, memory used of total, video memory and temperature) and a graph of the last two minutes. Under them, disk and network each show two speeds and draw both in one graph: read and write, download and upload. Speeds count in B/s, KB/s, MB/s or GB/s, and their graphs scale to the fastest moment of the two minutes. Then come swap, the GPU's name, and the six busiest processes with their CPU, memory and disk use. The process list updates while the page is open.

A switch above the list orders it by CPU, memory or disk. Disk use is what a process read from and wrote to storage, per second, from `/proc/<pid>/io`. Linux lets you read that file only for your own processes, so other users' processes show no disk use and sort last. When a program's child process exits, Linux adds the child's bytes to the program's count, so a shell or terminal that ran a big copy shows a burst for one reading. Network use per process isn't shown: Linux keeps no per-process byte counts that a user can read, and getting them takes root.

Hovering one of your own processes shows **End**. Clicking it turns it into **Confirm**; clicking that sends SIGTERM, which asks the program to quit. A program that's still running 3 seconds later shows **Force**, which sends SIGKILL and stops it at once. Other users' processes have no button. `mochi ipc performance end <pid>` and `kill <pid>` do the same from a shell, and refuse processes that aren't yours.

A reading that stays over its **notice** level shows a short notice on the island, once, naming the busiest process, like "CPU at 92% · firefox" or "Memory at 88% · Discord uses 3.1 GB". One that stays over its **critical** level shows a red notice and puts a red bubble with the reading next to the island until it comes back down. A spike says nothing: a reading must stay up for `sustain_seconds`. After a notice, a reading must fall 10 under the level before it can notify again, so one hovering at the level stays quiet. Disk and network speeds have no levels.

```toml
{{#include ../../../../modules/performance/settings.toml}}
```

GPU use defaults to a notice only at 95% and no critical level, since a game keeps the GPU at full use. Clicking the bubble opens the page. `mochi ipc performance status` prints the readings.

The CPU's temperature comes from hwmon (k10temp or zenpower on AMD, coretemp on Intel). An AMD GPU is read from sysfs; an NVIDIA GPU through one long-running `nvidia-smi`, which comes with NVIDIA's driver. Intel GPUs aren't read yet.

Disk speeds add up the whole disks in `/proc/diskstats` that have a `device` link in `/sys/block`. That leaves out partitions, which the disk already counts, and loop, RAM, zram, device-mapper and RAID devices, which pass their traffic on to a disk. Network speeds add up the interfaces in `/proc/net/dev` that have a `device` link in `/sys/class/net`: wired and wireless cards. That leaves out loopback, bridges, containers' virtual cards and VPN tunnels, whose traffic crosses a card anyway.

The [performance widget](widgets.md) shows the same graphs, each turned on or off in its settings.
