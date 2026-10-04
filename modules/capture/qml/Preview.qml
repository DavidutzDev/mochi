import QtQuick
import Quickshell.Widgets
import qs.island

// A capture just saved: the screenshot, or a film icon for a recording,
// with buttons to copy, edit, open the folder or delete. Or what went
// wrong. The clipboard shows its images here too, with a title of its own
// and no folder.
Item {
    id: root

    property var payload: ({})
    readonly property bool screenshot: payload.kind === "screenshot"
    readonly property bool failed: payload.error != null

    implicitWidth: 440
    implicitHeight: row.implicitHeight + Theme.padding * 2

    Row {
        id: row

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        spacing: 14

        ClippingRectangle {
            id: thumbnail

            width: 128
            height: 80
            radius: Theme.radiusMedium
            color: Theme.raised

            Symbol {
                anchors.centerIn: parent
                visible: !root.screenshot || root.failed || image.status !== Image.Ready
                name: root.failed ? "close" : root.screenshot ? "camera" : "video"
                size: 28
                color: root.failed ? Theme.danger : Theme.muted
            }

            Image {
                id: image

                anchors.fill: parent
                visible: root.screenshot && !root.failed
                source: visible && root.payload.path ? `file://${root.payload.path}` : ""
                sourceSize.width: 256
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: false
            }
        }

        Column {
            width: parent.width - thumbnail.width - parent.spacing
            spacing: 4

            Text {
                width: parent.width
                text: {
                    if (root.payload.title)
                        return root.payload.title;
                    const what = root.screenshot ? "Screenshot" : "Recording";
                    return root.failed ? `${what} failed` : `${what} saved`;
                }
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Theme.textSubtitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                text: root.failed ? root.payload.error : root.payload.copied ? `${root.payload.name} · copied` : root.payload.name ?? ""
                elide: root.failed ? Text.ElideRight : Text.ElideMiddle
                maximumLineCount: root.failed ? 3 : 1
                wrapMode: root.failed ? Text.Wrap : Text.NoWrap
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }

            Item {
                width: 1
                height: 4
            }

            Row {
                visible: !root.failed
                spacing: 6

                IconButton {
                    icon: "copy"
                    tone: "neutral"
                    onClicked: Daemon.command("capture", "copy", [])
                }

                IconButton {
                    visible: root.payload.editable ?? false
                    icon: "edit"
                    tone: "neutral"
                    onClicked: {
                        Daemon.command("capture", "edit", []);
                        Daemon.event("dismiss");
                    }
                }

                IconButton {
                    // An image from the clipboard has no folder.
                    visible: root.payload.folder != null
                    icon: "folder"
                    tone: "neutral"
                    onClicked: {
                        Daemon.command("capture", "open", []);
                        Daemon.event("dismiss");
                    }
                }

                IconButton {
                    icon: "trash"
                    tone: "danger"
                    onClicked: Daemon.command("capture", "delete", [])
                }
            }
        }
    }
}
