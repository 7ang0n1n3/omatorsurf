import QtQuick
import QtQuick.Layouts
import qs.Commons

RowLayout {
  property string label: ""
  property string value: ""
  spacing: Style.spacing.controlGap
  Text {
    Layout.fillWidth: true
    text: parent.label
    textFormat: Text.PlainText
    color: Color.popups.text
    font.family: Style.font.family
    font.pixelSize: Style.font.body
  }
  Text {
    text: parent.value
    textFormat: Text.PlainText
    color: Color.popups.text
    font.family: Style.font.family
    font.pixelSize: Style.font.body
  }
}
