import QtQuick

// The profiler of `mochid --dev`, which a click on its bubble opens: what
// mochid, Quickshell and the programs they start cost right now, the last
// minute of CPU, the frames each window draws, the busiest threads and
// modules, and Record, which writes a report with a flame graph of mochid.
// CPU is in percent of one core.
Item {
    id: root

    // {cpu, wakeups, memory: {mochid, quickshell, helpers}, history: [cpu],
    // frames: [{output, surface, fps}], threads: [{process, thread, cpu}],
    // modules: [{module, cpu}], recording: seconds left or null, report}
    property var payload: ({})
    readonly property var groups: [
        {
            "key": "mochid",
            "name": "mochid",
            "color": Theme.foreground
        },
        {
            "key": "quickshell",
            "name": "Quickshell",
            "color": Theme.accent
        },
        {
            "key": "helpers",
            "name": "Helpers",
            "color": Theme.muted
        }
    ]
    readonly property bool recording: payload.recording !== null && payload.recording !== undefined

    function number(value: real): string {
        return value >= 10 ? value.toFixed(0) : value.toFixed(1);
    }

    implicitWidth: 520
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: Theme.spaceSmall

        PanelHeader {
            width: parent.width
            title: "Profiler"
            icon: "code"

            Button {
                text: "Close"
                onClicked: Daemon.event("dismiss")
            }
        }

        // The last minute, stacked: mochid, Quickshell, then helpers.
        Canvas {
            id: chart

            readonly property var history: root.payload.history ?? []

            width: parent.width
            height: 72
            onHistoryChanged: requestPaint()
            onWidthChanged: requestPaint()

            onPaint: {
                const context = getContext("2d");
                context.reset();
                const points = history;
                const total = point => root.groups.reduce((sum, group) => sum + (point[group.key] ?? 0), 0);
                const top = Math.max(5, ...points.map(total)) * 1.15;
                const step = width / 119;
                const left = width - (points.length - 1) * step;
                context.strokeStyle = Theme.raised;
                context.lineWidth = 1;
                context.beginPath();
                context.moveTo(0, height - 0.5);
                context.lineTo(width, height - 0.5);
                context.stroke();
                const below = points.map(() => 0);
                for (const group of root.groups) {
                    context.fillStyle = group.color;
                    context.beginPath();
                    points.forEach((point, i) => {
                        const y = height - (below[i] + (point[group.key] ?? 0)) / top * height;
                        if (i === 0)
                            context.moveTo(left + i * step, y);
                        else
                            context.lineTo(left + i * step, y);
                    });
                    for (let i = points.length - 1; i >= 0; i--)
                        context.lineTo(left + i * step, height - below[i] / top * height);
                    context.closePath();
                    context.fill();
                    points.forEach((point, i) => below[i] += point[group.key] ?? 0);
                }
                context.fillStyle = Theme.muted;
                context.font = `${Theme.textCaption}px ${Theme.fontFamily}`;
                context.fillText(`${root.number(top)}%`, 2, Theme.textCaption);
            }
        }

        // Now: each process's CPU, memory and wake-ups.
        Repeater {
            model: root.groups

            Row {
                id: line

                required property var modelData

                width: column.width
                spacing: Theme.spaceSmall

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 8
                    height: 8
                    radius: 2
                    color: line.modelData.color
                }

                Text {
                    width: 90
                    text: line.modelData.name
                    textFormat: Text.PlainText
                    color: Theme.foreground
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.textBody
                }

                Text {
                    width: column.width - 90 - 8 - Theme.spaceSmall * 2
                    horizontalAlignment: Text.AlignRight
                    text: {
                        const key = line.modelData.key;
                        const cpu = root.payload.cpu?.[key];
                        if (cpu === undefined)
                            return "…";
                        return `${root.number(cpu)}% · ${Math.round(root.payload.memory?.[key] ?? 0)} MB · ${Math.round(root.payload.wakeups?.[key] ?? 0)} wake-ups/s`;
                    }
                    textFormat: Text.PlainText
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.textBody
                    font.features: {
                        "tnum": 1
                    }
                }
            }
        }

        SectionLabel {
            text: "Frames each second"
            topPadding: Theme.spaceTiny
        }

        Text {
            width: column.width
            elide: Text.ElideRight
            text: (root.payload.frames ?? []).length === 0 ? "Counting…" : root.payload.frames.map(frame => `${frame.output} ${frame.surface} ${frame.fps}`).join("   ")
            textFormat: Text.PlainText
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.textBody
        }

        SectionLabel {
            text: "Busiest threads and modules"
            topPadding: Theme.spaceTiny
        }

        Repeater {
            // Always seven rows, padded, so the panel keeps its height: a
            // list that grows and shrinks would make the island move.
            model: {
                const rows = [...(root.payload.threads ?? []).slice(0, 4).map(thread => ({
                                "name": `${thread.process.replace(/^\./, "")} › ${thread.thread}`,
                                "cpu": thread.cpu
                            })), ...(root.payload.modules ?? []).slice(0, 3).map(module => ({
                                "name": `module ${module.module}`,
                                "cpu": module.cpu
                            }))];
                while (rows.length < 7)
                    rows.push({
                        "name": "",
                        "cpu": null
                    });
                return rows;
            }

            Row {
                id: busy

                required property var modelData

                width: column.width

                Text {
                    width: parent.width - 60
                    elide: Text.ElideRight
                    text: busy.modelData.name
                    textFormat: Text.PlainText
                    color: Theme.foreground
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.textCaption
                }

                Text {
                    width: 60
                    horizontalAlignment: Text.AlignRight
                    text: busy.modelData.cpu === null ? "" : `${busy.modelData.cpu.toFixed(2)}%`
                    textFormat: Text.PlainText
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.textCaption
                    font.features: {
                        "tnum": 1
                    }
                }
            }
        }

        Row {
            topPadding: Theme.spaceSmall
            spacing: Theme.spaceSmall

            Button {
                text: root.recording ? `Recording, ${root.payload.recording} s left` : "Record 30 s"
                icon: "record"
                tone: "accent"
                enabled: !root.recording
                onClicked: Daemon.send({
                    "type": "profile",
                    "seconds": 30
                })
            }

            Button {
                visible: !root.recording && (root.payload.report ?? "") !== ""
                text: "Open the report"
                onClicked: Qt.openUrlExternally(`file://${root.payload.report}`)
            }
        }
    }
}
