import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui

Ui.Panel {
  id: root
  moduleName: "io.github.7ang0n1n3.omatorsurf"
  manageIpc: false
  readonly property bool vertical: bar ? bar.vertical : false
  readonly property string toggleText: backend.busy ? backend.statusText : backend.status ? (backend.tablePresent ? "Stop" : "Start") : "Start"
  implicitWidth: vertical ? (bar ? bar.barSize : Style.bar.sizeVertical) : statusButton.implicitWidth
  implicitHeight: vertical ? statusButton.implicitHeight : (bar ? bar.barSize : Style.bar.sizeHorizontal)

  Backend { id: backend }

  Ui.BarIconButton {
    id: statusButton
    anchors.fill: parent
    bar: root.bar
    // User-selected Nerd Font codepoints: Off F199A / On F0CCC.
    text: backend.protectedRoute ? String.fromCodePoint(0xF0CCC) : String.fromCodePoint(0xF199A)
    active: backend.status !== null && backend.status.protection_state === "degraded"
    tooltipText: "Omatorsurf: " + backend.statusText + "\nClick for controls and public IP"
    onPressed: function(button) {
      if (button === Qt.RightButton) backend.refresh()
      else root.toggle()
    }
  }

  Ui.KeyboardPanel {
    id: popup
    anchorItem: statusButton
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: closeButton
    contentWidth: popup.fittedContentWidth(Style.space(420))
    contentHeight: popup.fittedContentHeight(content.implicitHeight, Style.space(600))

    Flickable {
      anchors.fill: parent
      contentHeight: content.implicitHeight
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      Keys.onEscapePressed: root.close()

      ColumnLayout {
        id: content
        width: parent.width
        spacing: Style.spacing.panelGap

        RowLayout {
          Layout.fillWidth: true
          Text {
            Layout.fillWidth: true
            text: "Omatorsurf"
            textFormat: Text.PlainText
            color: Color.popups.text
            font.family: Style.font.family
            font.pixelSize: Style.font.heading
          }
          Ui.Button {
            id: closeButton
            text: "Close"
            foreground: Color.popups.text
            focusable: true
            onClicked: root.close()
          }
        }
        Ui.PanelSectionHeader { text: "PROTECTION"; foreground: Color.popups.text }
        StatusRow { Layout.fillWidth: true; label: "Status"; value: backend.statusText }
        StatusRow { Layout.fillWidth: true; label: backend.protectedRoute ? "Tor exit IPv4" : "Public IPv4"; value: backend.publicIp }
        Repeater {
          model: [
            { label: "Tor ready", key: "tor_running" },
            { label: "Firewall verified", key: "firewall_active" },
            { label: "Kill switch", key: "killswitch" },
            { label: "DNS protected", key: "dns_protected" },
            { label: "IPv6 blocked", key: "ipv6_protected" }
          ]
          StatusRow {
            required property var modelData
            Layout.fillWidth: true
            label: modelData.label
            value: backend.busy || !backend.status ? "Unknown" : backend.status[modelData.key] ? "Yes" : "No"
          }
        }
        StatusRow {
          Layout.fillWidth: true
          label: "Observation age"
          value: backend.observation ? Math.max(0, Math.floor(backend.age)) + "s" : "Unavailable"
        }
        Ui.PanelSeparator { Layout.fillWidth: true; foreground: Color.popups.text }
        Flow {
          Layout.fillWidth: true
          spacing: Style.spacing.controlGap
          Ui.Button {
            text: root.toggleText
            foreground: Color.popups.text
            focusable: true
            bordered: true
            enabled: !backend.busy && backend.status !== null
            opacity: enabled ? 1 : 0.45
            onClicked: backend.act(backend.tablePresent ? "stop" : "start")
          }
          Ui.Button {
            text: "New circuit"
            foreground: Color.popups.text
            focusable: true
            bordered: true
            enabled: backend.protectedRoute && !backend.busy
            opacity: enabled ? 1 : 0.45
            onClicked: backend.act("new-circuit")
          }
          Ui.Button {
            visible: !backend.status && !backend.busy
            text: "Stop / restore network"
            foreground: Color.popups.text
            focusable: true
            bordered: true
            tooltipText: "Explicitly remove the Omatorsurf guard even when status is unavailable"
            onClicked: backend.act("stop")
          }
        }
        Text {
          Layout.fillWidth: true
          text: "New circuits affect future connections. Existing streams remain open and a different exit IP is not guaranteed."
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          color: Color.popups.text
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }
        Text {
          Layout.fillWidth: true
          visible: backend.details !== ""
          text: backend.details
          textFormat: Text.PlainText
          wrapMode: Text.WrapAnywhere
          color: Color.popups.text
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }
      }
    }
  }
}
