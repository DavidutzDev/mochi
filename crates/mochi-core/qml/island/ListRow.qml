import QtQuick

// A row in a list: something at the start (an `icon`, an `image`, or any
// item put in `leading`), a title and subtitle, and anything put in
// `trailing`, like a close button. `flat` rows have no fill until hovered or
// `selected`; `marker` draws the accent bar on the selected one.
Rectangle {
    id: root

    property string icon: ""
    property string image: ""
    property string title: ""
    property string subtitle: ""
    // Text.StyledText for a subtitle with markup; its links emit
    // `linkActivated` instead of `clicked`.
    property int subtitleFormat: Text.PlainText
    property bool selected: false
    property bool flat: false
    property bool marker: false
    property real leadingSize: 34
    // A file the row can be dragged out as and dropped on any app, as from
    // a file manager: its path. The drag shows what leads the row.
    property string file: ""
    // What a drag of the row carries: its file, or a page's selection when
    // the row is in it, with their number on the picture.
    property var files: file === "" ? [] : [file]
    // Shift+click or Ctrl+click says `selectionToggled` instead of `clicked`, for
    // the page to add the row to its selection or take it off.
    property bool selectable: false
    property alias leading: leadingSlot.data
    property alias trailing: trailingSlot.data
    readonly property bool hovered: area.containsMouse
    signal clicked
    signal selectionToggled
    signal linkActivated(string link)

    implicitWidth: 360
    implicitHeight: 56
    radius: Theme.radiusField
    color: selected ? Theme.raised : hovered ? (flat ? Theme.surface : Theme.raised) : flat ? "transparent" : Theme.surface

    Behavior on color {
        ColorAnimation {
            duration: Theme.fast
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: mouse => {
            if (area.dragged)
                return;
            if (root.selectable && mouse.modifiers & (Qt.ShiftModifier | Qt.ControlModifier))
                root.selectionToggled();
            else
                root.clicked();
        }

        // Held and moved, a row with files is dragged out, with what leads
        // it drawn under the pointer; DragOut carries it from there. A
        // click stays a click.
        property point pressedAt
        property bool dragged: false

        onPressed: mouse => {
            area.pressedAt = Qt.point(mouse.x, mouse.y);
            area.dragged = false;
        }
        onPositionChanged: mouse => {
            if (!area.pressed || area.dragged || root.files.length === 0)
                return;
            if (Math.hypot(mouse.x - area.pressedAt.x, mouse.y - area.pressedAt.y) < Qt.styleHints.startDragDistance)
                return;
            area.dragged = true;
            // The picture, with the count on it, once it's drawn.
            const files = root.files;
            const window = root.Window.window;
            const hotSpot = Qt.point(root.leadingSize / 2, root.leadingSize / 2);
            leadingSlot.grabToImage(picture => DragOut.start(window, files, picture, hotSpot));
        }
    }

    Rectangle {
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        visible: root.marker && root.selected
        width: 3
        height: parent.height - 20
        radius: 1.5
        color: Theme.accent
    }

    Item {
        id: leadingSlot

        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: root.leadingSize
        height: root.leadingSize

        // How many files a drag carries, on its picture, over what the
        // page put here.
        Badge {
            z: 10
            anchors.right: parent.right
            anchors.top: parent.top
            visible: area.dragged && root.files.length > 1
            count: root.files.length
        }

        Symbol {
            anchors.centerIn: parent
            visible: root.icon !== ""
            name: root.icon
            size: root.leadingSize * 0.6
            color: Theme.foreground
        }

        Image {
            anchors.fill: parent
            visible: root.image !== ""
            source: root.image
            sourceSize.width: root.leadingSize * 2
            sourceSize.height: root.leadingSize * 2
            fillMode: Image.PreserveAspectFit
            // A theme icon loads here: Qt's icon themes break when read from
            // the loading thread while this one reads them too. Files load
            // in the background.
            asynchronous: !String(source).startsWith("image://icon/")
        }
    }

    Column {
        anchors.left: leadingSlot.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: trailingSlot.left
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }

        Text {
            width: parent.width
            visible: text !== ""
            text: root.subtitle
            elide: Text.ElideRight
            textFormat: root.subtitleFormat
            color: Theme.muted
            linkColor: Theme.accent
            onLinkActivated: link => root.linkActivated(link)
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Row {
        id: trailingSlot

        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceSmall
    }
}
