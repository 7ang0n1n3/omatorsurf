import QtQuick
import Quickshell.Io

// One reader per bar surface; the system timer performs the expensive observation
// once for the entire machine. No authentication is needed for reading snapshots.
Item {
  id: root
  property var observation: null
  property double now: Date.now() / 1000
  property string readError: "Waiting for the status publisher"
  property string actionError: ""
  property string actionName: ""
  readonly property bool actionRunning: actionProcess.running
  readonly property double age: observation ? now - observation.checked_at : Infinity
  readonly property bool fresh: observation !== null && age >= -5 && age <= 90
  readonly property var status: fresh && observation.status ? observation.status : null
  readonly property string publisherBusy: observation && age >= -5 && age <= 930 ? observation.busy : ""
  readonly property bool busy: actionRunning || publisherBusy !== ""
  readonly property bool protectedRoute: !busy && status !== null && status.enabled
  readonly property bool tablePresent: status !== null && status.protection_state !== "disabled"
  readonly property string publicIp: !busy && status && observation.current_ip ? observation.current_ip : "Unknown"
  readonly property string statusText: {
    var action = actionRunning ? actionName : publisherBusy
    if (action === "start") return "Starting"
    if (action === "stop") return "Stopping"
    if (action === "new-circuit") return "Changing circuit"
    if (!status) return "Unknown"
    if (status.protection_state === "protected") return "Protected"
    if (status.protection_state === "degraded") return "Degraded"
    return "Off"
  }
  readonly property string details: {
    var errors = []
    if (actionError) errors.push(actionError)
    if (readError) errors.push(readError)
    if (observation && !fresh && !publisherBusy) errors.push("The observation is stale. Protection is not verified.")
    if (observation && observation.error) errors.push(observation.error)
    if (status && status.errors) errors = errors.concat(status.errors)
    if (observation && observation.ip_error) errors.push("IP lookup: " + observation.ip_error)
    return errors.join("\n\n")
  }

  function refresh() {
    if (!reader.running) reader.running = true
  }

  function act(command) {
    if (busy || ["start", "stop", "new-circuit"].indexOf(command) < 0) return
    if (command === "new-circuit" && !protectedRoute) return
    actionError = ""
    actionName = command
    // Fixed installed helper and argument array: no command interpolation.
    actionProcess.command = ["/usr/bin/pkexec", "/usr/local/libexec/omatorsurf-bar", command]
    actionProcess.running = true
  }

  function accept(data) {
    var value = JSON.parse(data)
    if (!value || value.schema_version !== 1 || typeof value.checked_at !== "number"
        || !isFinite(value.checked_at) || typeof value.busy !== "string"
        || ["", "start", "stop", "new-circuit"].indexOf(value.busy) < 0
        || typeof value.error !== "string" || typeof value.ip_error !== "string")
      throw new Error("Invalid status observation")
    var s = value.status
    if (s !== null) {
      if (!s || ["protected", "degraded", "disabled"].indexOf(s.protection_state) < 0
          || !Array.isArray(s.errors)) throw new Error("Invalid backend status")
      var flags = ["enabled", "tor_running", "firewall_active", "killswitch", "dns_protected", "ipv6_protected"]
      for (var i = 0; i < flags.length; i++)
        if (typeof s[flags[i]] !== "boolean") throw new Error("Invalid backend status flag")
      if (s.enabled !== (s.protection_state === "protected"))
        throw new Error("Inconsistent protection state")
      if (s.enabled && (s.protection_state !== "protected" || !s.public_ip
          || !s.tor_running || !s.firewall_active || !s.killswitch || !s.dns_protected || !s.ipv6_protected))
        throw new Error("Inconsistent protection observation")
    }
    if (value.current_ip !== null && typeof value.current_ip !== "string")
      throw new Error("Invalid public IP observation")
    observation = value
    readError = ""
  }

  Process {
    id: reader
    command: ["/usr/bin/timeout", "--kill-after=1s", "3s", "/usr/local/libexec/omatorsurf-bar", "read"]
    stdout: StdioCollector { id: readOutput; waitForEnd: true }
    stderr: StdioCollector { id: readDiagnostics; waitForEnd: true }
    onExited: function(code, exitStatus) {
      if (code === 0 && exitStatus === 0) {
        try { root.accept(readOutput.text); return }
        catch (error) { root.readError = String(error) }
      } else {
        root.readError = readDiagnostics.text.trim() || "Status publisher unavailable. Install the plugin's system integration."
      }
      root.observation = null
    }
  }

  Process {
    id: actionProcess
    command: []
    stderr: StdioCollector { id: actionDiagnostics; waitForEnd: true }
    onExited: function(code, exitStatus) {
      if (code !== 0 || exitStatus !== 0)
        root.actionError = (code === 126 ? "Authentication was cancelled." :
          code === 127 ? "Authentication failed or pkexec is unavailable." :
          actionDiagnostics.text.trim() || "The operation failed. Inspect the status details.")
      root.actionName = ""
      root.refresh()
    }
  }

  Timer {
    interval: 2000
    repeat: true
    running: true
    triggeredOnStart: true
    onTriggered: { root.now = Date.now() / 1000; root.refresh() }
  }
}
