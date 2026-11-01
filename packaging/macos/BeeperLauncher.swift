// Finder entry for the Beeper fork. No shell, Terminal, bundled secret or Server setup.
import AppKit
import Foundation

struct Connection: Codable {
    let host: String
    let user: String
    let allowWrites: Bool
}

enum LaunchError: Error { case invalid, ssh, target, tunnel, missingBinary }
let manager = FileManager.default
let profile = manager.homeDirectoryForCurrentUser.appendingPathComponent(".zapfast-beeper-prototype")
let configURL = profile.appendingPathComponent("launcher.json")
let app = NSApplication.shared
app.setActivationPolicy(.accessory)

func showError() {
    let alert = NSAlert()
    alert.messageText = "ZapFast could not connect to the existing Server"
    alert.informativeText = "Check the Mac Studio's SSH address, your existing SSH key access and that Server is running. Close an old tunnel using port 23374 before retrying. No account was reset or linked. No private response was logged."
    alert.runModal()
}

func valid(_ value: String, pattern: String) -> Bool {
    value.range(of: pattern, options: .regularExpression) != nil
}

func configure() -> Connection? {
    let saved = (try? Data(contentsOf: configURL)).flatMap { try? JSONDecoder().decode(Connection.self, from: $0) }
    // Autojoin: a saved connection launches without asking, so opening the app
    // joins the Server directly. The setup alert stays reachable on purpose:
    // pass `--reconfigure` or hold Option while the app opens.
    if let saved,
       !CommandLine.arguments.contains("--reconfigure"),
       !NSEvent.modifierFlags.contains(.option) { return saved }
    let alert = NSAlert()
    alert.messageText = "Connect to your existing Beeper Server"
    alert.informativeText = "Server runs on your other Mac. Use its SSH hostname or Tailscale IP and your SSH username. Existing key access is required; this app never asks for or saves an SSH password. It does not install Server or link networks."
    alert.addButton(withTitle: "Connect")
    alert.addButton(withTitle: "Cancel")
    let box = NSView(frame: NSRect(x: 0, y: 0, width: 380, height: 115))
    let host = NSTextField(string: saved?.host ?? "")
    host.placeholderString = "Server Mac hostname or IP"
    host.frame = NSRect(x: 0, y: 80, width: 380, height: 24)
    let user = NSTextField(string: saved?.user ?? "")
    user.placeholderString = "SSH username on the Server Mac"
    user.frame = NSRect(x: 0, y: 45, width: 380, height: 24)
    let writes = NSButton(checkboxWithTitle: "Enable sending, typing and marking chats read", target: nil, action: nil)
    writes.state = saved?.allowWrites == true ? .on : .off
    writes.frame = NSRect(x: 0, y: 5, width: 380, height: 26)
    box.addSubview(host); box.addSubview(user); box.addSubview(writes)
    alert.accessoryView = box
    app.activate(ignoringOtherApps: true)
    guard alert.runModal() == .alertFirstButtonReturn else { return nil }
    let result = Connection(host: host.stringValue.trimmingCharacters(in: .whitespacesAndNewlines),
                            user: user.stringValue.trimmingCharacters(in: .whitespacesAndNewlines),
                            allowWrites: writes.state == .on)
    guard valid(result.host, pattern: "^[A-Za-z0-9][A-Za-z0-9.:-]{0,252}$"),
          valid(result.user, pattern: "^[A-Za-z_][A-Za-z0-9_.-]{0,63}$") else { return nil }
    return result
}

func ssh(_ connection: Connection, extra: [String]) -> Process {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/ssh")
    process.arguments = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-o", "StrictHostKeyChecking=yes",
                         "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2"] + extra +
                        ["\(connection.user)@\(connection.host)"]
    process.standardInput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    return process
}

func stop(_ process: Process) {
    if process.isRunning { process.terminate() }
}

func launch(_ connection: Connection) throws {
    // Read one named target using the user's pre-existing SSH trust. Fixed command only.
    let query = ssh(connection, extra: [])
    query.arguments! += ["PATH=/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin /opt/homebrew/bin/beeper targets show server --json"]
    let output = Pipe()
    query.standardOutput = output
    try query.run()
    let timeout = DispatchWorkItem { stop(query) }
    DispatchQueue.global().asyncAfter(deadline: .now() + 15, execute: timeout)
    defer { timeout.cancel() }
    var bytes = Data()
    while let chunk = try output.fileHandleForReading.read(upToCount: 65536), !chunk.isEmpty {
        guard bytes.count + chunk.count <= 1024 * 1024 else {
            stop(query); throw LaunchError.target
        }
        bytes.append(chunk)
    }
    query.waitUntilExit()
    let tooLarge = bytes.count > 1024 * 1024
    guard query.terminationStatus == 0, !tooLarge,
          let envelope = try JSONSerialization.jsonObject(with: bytes) as? [String: Any],
          envelope["success"] as? Bool != false else { throw LaunchError.target }
    let target = envelope["data"] as? [String: Any] ?? envelope
    guard target["id"] as? String == "server",
          let base = target["baseURL"] as? String,
          let url = URLComponents(string: base), url.scheme == "http", url.host == "127.0.0.1", url.port == 23374,
          url.path.isEmpty || url.path == "/", url.query == nil, url.fragment == nil,
          url.user == nil, url.password == nil,
          let auth = target["auth"] as? [String: Any], let token = auth["accessToken"] as? String,
          !token.isEmpty, token.count <= 16384, !token.contains("\r"), !token.contains("\n")
    else { throw LaunchError.target }

    // The remote ready marker is emitted only after SSH has established forwarding.
    // Merely checking isRunning before authentication could send a token to an old listener.
    let tunnel = ssh(connection, extra: ["-T", "-o", "ExitOnForwardFailure=yes", "-L", "127.0.0.1:23374:127.0.0.1:23374"])
    tunnel.arguments! += ["printf READY; cat >/dev/null"]
    let hold = Pipe()
    let ready = Pipe()
    tunnel.standardInput = hold
    tunnel.standardOutput = ready
    try tunnel.run()
    defer { stop(tunnel) }
    let tunnelTimeout = DispatchWorkItem { stop(tunnel) }
    DispatchQueue.global().asyncAfter(deadline: .now() + 15, execute: tunnelTimeout)
    var marker = Data()
    while marker.count < 5 {
        guard let chunk = try ready.fileHandleForReading.read(upToCount: 5 - marker.count), !chunk.isEmpty else { break }
        marker.append(chunk)
    }
    tunnelTimeout.cancel()
    guard tunnel.isRunning, marker == Data("READY".utf8) else { throw LaunchError.tunnel }
    let binary = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/zapfast-client")
    guard manager.isExecutableFile(atPath: binary.path) else { throw LaunchError.missingBinary }
    try manager.createDirectory(at: profile, withIntermediateDirectories: true,
                                attributes: [.posixPermissions: 0o700])
    // Only SSH host/user and the explicit write-mode choice persist. Never the API token.
    try JSONEncoder().encode(connection).write(to: configURL, options: .atomic)
    try manager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: configURL.path)
    let client = Process()
    client.executableURL = binary
    client.arguments = Array(CommandLine.arguments.dropFirst()).filter { !$0.hasPrefix("-psn_") }
    var env = ProcessInfo.processInfo.environment
    for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] { env.removeValue(forKey: key) }
    env["NO_PROXY"] = "127.0.0.1,localhost,::1"
    env["no_proxy"] = "127.0.0.1,localhost,::1"
    env["ZAPFAST_BACKEND"] = "beeper"
    env["ZAPFAST_PROFILE_ROOT"] = profile.path
    env["BEEPER_DESKTOP_BASE_URL"] = "http://127.0.0.1:23374"
    env["BEEPER_ACCESS_TOKEN"] = token
    env["BEEPER_READONLY"] = connection.allowWrites ? "0" : "1"
    env["BEEPER_SIGN_IN"] = "0"
    client.environment = env
    client.standardOutput = FileHandle.nullDevice
    client.standardError = FileHandle.nullDevice
    try client.run()
    while client.isRunning {
        if !tunnel.isRunning { stop(client); throw LaunchError.tunnel }
        Thread.sleep(forTimeInterval: 0.25)
    }
    client.waitUntilExit()
}

if let connection = configure() {
    do { try launch(connection) } catch { showError(); exit(1) }
}
