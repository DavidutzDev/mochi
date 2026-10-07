import QtQuick

// Text whose digits roll when they change, like a clock's minutes or a
// percentage: the old digit slides out as the new one comes in, upward
// when the number grows. Other characters change in place. Digits all take
// the width of a 0, so the text doesn't jiggle.
Item {
    id: root

    property string text: ""
    property color color: Theme.foreground
    property int pixelSize: Theme.textBody
    property string family: Theme.fontFamily
    property int weight: Theme.weightBody
    // Off for the first value and for instant changes.
    property bool animated: true

    readonly property font font: Qt.font({
        "family": family,
        "pixelSize": pixelSize,
        "weight": weight,
        "features": {
            "tnum": 1
        }
    })
    // Whether the last change made the number bigger, for the direction.
    property bool rising: true
    property string previous: ""
    onTextChanged: {
        const before = parseFloat(previous.replace(/[^0-9.-]/g, ""));
        const after = parseFloat(text.replace(/[^0-9.-]/g, ""));
        rising = isNaN(before) || isNaN(after) || after >= before;
        previous = text;
    }
    Component.onCompleted: previous = text

    implicitWidth: row.implicitWidth
    implicitHeight: zero.height

    TextMetrics {
        id: zero

        font: root.font
        text: "0"
    }

    Row {
        id: row

        Repeater {
            model: root.text.length

            Item {
                id: slot

                required property int index
                readonly property string character: root.text.charAt(index)
                readonly property bool digit: /[0-9]/.test(character)
                property string shown: character
                property string leaving: ""
                // 0 when a change starts, 1 when the new character is in.
                property real progress: 1

                width: digit ? zero.advanceWidth : metrics.advanceWidth
                height: zero.height
                clip: true

                TextMetrics {
                    id: metrics

                    font: root.font
                    text: slot.character
                }

                onCharacterChanged: {
                    if (!root.animated || !digit || shown === "" || !visible) {
                        shown = character;
                        leaving = "";
                        progress = 1;
                        return;
                    }
                    leaving = shown;
                    shown = character;
                    roll.restart();
                }

                NumberAnimation {
                    id: roll

                    target: slot
                    property: "progress"
                    from: 0
                    to: 1
                    duration: Theme.move
                    easing.type: Easing.OutCubic
                    onFinished: slot.leaving = ""
                }

                Text {
                    y: (root.rising ? -1 : 1) * slot.height * slot.progress
                    visible: slot.leaving !== ""
                    text: slot.leaving
                    color: root.color
                    font: root.font
                    opacity: 1 - slot.progress
                }

                Text {
                    y: (root.rising ? 1 : -1) * slot.height * (1 - slot.progress)
                    text: slot.shown
                    color: root.color
                    font: root.font
                }
            }
        }
    }
}
