# Performance

CPU, memory and GPU use, and the CPU's and GPU's temperatures.

The hub has a **Performance page**: CPU, memory and GPU, each with its value, a detail line (threads and temperature, memory used of total, video memory and temperature) and a graph of the last two minutes, then swap, the GPU's name, and the six busiest processes with their CPU and memory. The process list updates while the page is open.

A reading that stays over its **notice** level shows a short notice on the island, once, naming the busiest process, like "CPU at 92% · firefox" or "Memory at 88% · Discord uses 3.1 GB". One that stays over its **critical** level shows a red notice and puts a red bubble with the reading next to the island until it comes back down. A spike says nothing: a reading must stay up for `sustain_seconds`. After a notice, a reading must fall 10 under the level before it can notify again, so one hovering at the level stays quiet.

```toml
{{#include ../../../../modules/performance/settings.toml}}
```

GPU use defaults to a notice only at 95% and no critical level, since a game keeps the GPU at full use. Clicking the bubble opens the page. `mochi ipc performance status` prints the readings.

The CPU's temperature comes from hwmon (k10temp or zenpower on AMD, coretemp on Intel). An AMD GPU is read from sysfs; an NVIDIA GPU through one long-running `nvidia-smi`, which comes with NVIDIA's driver. Intel GPUs aren't read yet.
