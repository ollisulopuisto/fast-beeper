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

let schedulerHealthScript = (try? String(contentsOf:
    Bundle.main.bundleURL.appendingPathComponent("Contents/Resources/scheduler-health.sh"),
    encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "false"

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

    // Send Later setup: the Server Mac needs the scheduler daemon for
    // scheduled sends while this app is closed. Offer a one-time install of
    // the bundled daemon; nothing is installed or started without approval.
    func offerSchedulerInstall(_ connection: Connection, beeperToken: String) {
        let bundled = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/zapfast-scheduler")
        guard manager.isExecutableFile(atPath: bundled.path) else { return }
        let declined = profile.appendingPathComponent("scheduler-install-declined")
        let reconfigure = CommandLine.arguments.contains("--reconfigure")
            || NSEvent.modifierFlags.contains(.option)
        guard reconfigure || !manager.fileExists(atPath: declined.path) else { return }
        let check = ssh(connection, extra: [])
        check.arguments! += [schedulerHealthScript + " && echo HEALTHY || true"]
        let checkOut = Pipe()
        check.standardOutput = checkOut
        guard (try? check.run()) != nil else { return }
        let checkTimeout = DispatchWorkItem { stop(check) }
        DispatchQueue.global().asyncAfter(deadline: .now() + 10, execute: checkTimeout)
        let checkData = checkOut.fileHandleForReading.readDataToEndOfFile()
        checkTimeout.cancel()
        check.waitUntilExit()
        guard check.terminationStatus == 0,
              !String(decoding: checkData, as: UTF8.self).contains("HEALTHY") else { return }
        let alert = NSAlert()
        alert.messageText = "Set up Send Later on \(connection.host)?"
        alert.informativeText = "ZapFast can copy its scheduler to the Server Mac and start it as a login agent, so scheduled messages send even when this app is closed. You can remove it later (see contrib/send-later). Don't Ask Again hides this until you relaunch while holding Option."
        alert.addButton(withTitle: "Install")
        alert.addButton(withTitle: "Later")
        alert.addButton(withTitle: "Don't Ask Again")
        app.activate(ignoringOtherApps: true)
        let choice = alert.runModal()
        if choice == .alertThirdButtonReturn {
            try? Data().write(to: declined)
            return
        }
        guard choice == .alertFirstButtonReturn else { return }
        installScheduler(connection, bundled: bundled, beeperToken: beeperToken)
    }

    // Runs a script on the Server Mac over the existing SSH trust. The
    // script travels as one argument; only file payloads travel on stdin.
    func remote(_ connection: Connection, _ script: String, stdin: Data? = nil) -> (Int32, Data) {
        let process = ssh(connection, extra: [])
        process.arguments! += ["set -e\n" + script]
        let out = Pipe()
        process.standardOutput = out
        let input = stdin.map { _ in Pipe() }
        if let input { process.standardInput = input }
        guard (try? process.run()) != nil else { return (-1, Data()) }
        let timeout = DispatchWorkItem { stop(process) }
        DispatchQueue.global().asyncAfter(deadline: .now() + 30, execute: timeout)
        if let input, let stdin {
            try? input.fileHandleForWriting.write(contentsOf: stdin)
            try? input.fileHandleForWriting.close()
        }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        timeout.cancel()
        process.waitUntilExit()
        return (process.terminationStatus, data)
    }

    // Mirrors contrib/send-later/install_macos.sh over SSH: bundled daemon,
    // 0600 control token, launchd agent. Tokens never travel on argv.
    func installScheduler(_ connection: Connection, bundled: URL, beeperToken: String) {
        let arch = remote(connection, "uname -m")
        guard arch.0 == 0,
              String(decoding: arch.1, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines) == "arm64" else {
            let alert = NSAlert()
            alert.messageText = "Send Later needs an Apple Silicon Server Mac"
            alert.informativeText = "The bundled scheduler runs on Apple Silicon only. Send Later stays off; everything else keeps working."
            alert.runModal()
            return
        }
        guard let binaryData = try? Data(contentsOf: bundled) else { return }
        let copy = remote(connection, """
        mkdir -p "$HOME/Library/Application Support/zapfast/bin" "$HOME/Library/LaunchAgents" "$HOME/.config/zapfast"
        chmod 700 "$HOME/Library/Application Support/zapfast" "$HOME/.config/zapfast"
        umask 077
        stage=$(mktemp "$HOME/Library/Application Support/zapfast/bin/scheduler.XXXXXX")
        trap 'rm -f "$stage"' EXIT
        cat > "$stage"
        chmod 755 "$stage"
        mv "$stage" "$HOME/Library/Application Support/zapfast/bin/zapfast-scheduler"
        """, stdin: binaryData)
        guard copy.0 == 0 else { installFailed(connection, stage: "copying the scheduler") ; return }
        let tokens = remote(connection, """
        umask 077
        APP_SUPPORT="$HOME/Library/Application Support/zapfast"
        [ -f "$APP_SUPPORT/scheduler-token" ] || openssl rand -hex 32 > "$APP_SUPPORT/scheduler-token"
        chmod 600 "$APP_SUPPORT/scheduler-token"
        ln -sf "$APP_SUPPORT/scheduler-token" "$HOME/.config/zapfast/scheduler-token"
        [ -f "$APP_SUPPORT/beeper-token" ] && echo HAVE || echo NEED
        """)
        guard tokens.0 == 0 else { installFailed(connection, stage: "writing the control token"); return }
        if String(decoding: tokens.1, as: UTF8.self).contains("NEED") {
            let apiTokenWrite = remote(connection, "umask 077 && cat > \"$HOME/Library/Application Support/zapfast/beeper-token\"",
                       stdin: Data(beeperToken.utf8))
            guard apiTokenWrite.0 == 0 else {
                installFailed(connection, stage: "writing the API token"); return
            }
        }
        let service = remote(connection, """
        APP_SUPPORT="$HOME/Library/Application Support/zapfast"
        PLIST="$HOME/Library/LaunchAgents/com.zapfast.scheduler.plist"
        cat > "$PLIST" <<PLIST
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
            <key>Label</key>
            <string>com.zapfast.scheduler</string>
            <key>ProgramArguments</key>
            <array>
                <string>$APP_SUPPORT/bin/zapfast-scheduler</string>
            </array>
            <key>EnvironmentVariables</key>
            <dict>
                <key>ZAPFAST_LISTEN</key>
                <string>127.0.0.1:23376</string>
                <key>BEEPER_BASE_URL</key>
                <string>http://127.0.0.1:23374</string>
                <key>BEEPER_ACCESS_TOKEN_FILE</key>
                <string>$APP_SUPPORT/beeper-token</string>
                <key>ZAPFAST_CONTROL_TOKEN_FILE</key>
                <string>$APP_SUPPORT/scheduler-token</string>
                <key>ZAPFAST_QUEUE_PATH</key>
                <string>$APP_SUPPORT/queue.json</string>
            </dict>
            <key>KeepAlive</key>
            <true/>
            <key>RunAtLoad</key>
            <true/>
            <key>StandardOutPath</key>
            <string>$APP_SUPPORT/scheduler.log</string>
            <key>StandardErrorPath</key>
            <string>$APP_SUPPORT/scheduler.log</string>
        </dict>
        </plist>
        PLIST
        launchctl unload "$PLIST" 2>/dev/null || true
        launchctl load "$PLIST"
        sleep 1
        """ + "\n" + schedulerHealthScript + " && echo RUNNING")
        if service.0 == 0, String(decoding: service.1, as: UTF8.self).contains("RUNNING") {
            let alert = NSAlert()
            alert.messageText = "Send Later is set up"
            alert.informativeText = "The scheduler is running on \(connection.host) as a login agent."
            alert.runModal()
        } else {
            installFailed(connection, stage: "starting the scheduler service")
        }
    }

    func installFailed(_ connection: Connection, stage: String) {
        let alert = NSAlert()
        alert.messageText = "Send Later setup did not finish"
        alert.informativeText = "Setup stopped while \(stage) on \(connection.host). Anything already copied stays there and is reused on a safe retry (relaunch while holding Option), or see contrib/send-later for manual install and removal."
        alert.runModal()
    }

    // The remote ready marker is emitted only after SSH has established forwarding.
    // Merely checking isRunning before authentication could send a token to an old listener.
    // Send Later rides the same SSH trust: forward the scheduler socket too,
    // but never let an occupied local 23376 break the Server connection.
    func openTunnel(_ forwards: [String]) -> (Process, Pipe)? {
        var extra = ["-T", "-o", "ExitOnForwardFailure=yes"]
        for forward in forwards { extra += ["-L", forward] }
        let tunnel = ssh(connection, extra: extra)
        tunnel.arguments! += ["printf READY; cat >/dev/null"]
        let hold = Pipe()
        let ready = Pipe()
        tunnel.standardInput = hold
        tunnel.standardOutput = ready
        do { try tunnel.run() } catch { return nil }
        let tunnelTimeout = DispatchWorkItem { stop(tunnel) }
        DispatchQueue.global().asyncAfter(deadline: .now() + 15, execute: tunnelTimeout)
        var marker = Data()
        while marker.count < 5 {
            guard let chunk = try? ready.fileHandleForReading.read(upToCount: 5 - marker.count), !chunk.isEmpty else { break }
            marker.append(chunk)
        }
        tunnelTimeout.cancel()
        guard tunnel.isRunning, marker == Data("READY".utf8) else { stop(tunnel); return nil }
        return (tunnel, hold)
    }
    let forwards = ["127.0.0.1:23374:127.0.0.1:23374", "127.0.0.1:23376:127.0.0.1:23376"]
    let opened: (Process, Pipe, Bool)?
    if let both = openTunnel(forwards) {
        opened = (both.0, both.1, true)
    } else if let apiOnly = openTunnel(Array(forwards.prefix(1))) {
        opened = (apiOnly.0, apiOnly.1, false)
    } else {
        opened = nil
    }
    guard let (tunnel, hold, schedulerForwarded) = opened else { throw LaunchError.tunnel }
    defer { stop(tunnel) }
    _ = hold
    if !schedulerForwarded {
        // Without our own 23376 listener the app must not send a previously
        // synced scheduler token to whatever occupies the local port.
        try? manager.removeItem(at: profile.appendingPathComponent("config/scheduler-token"))
    }

    offerSchedulerInstall(connection, beeperToken: token)

    // Best effort: the scheduler's control token lives only on the Server Mac
    // and reaches this Mac through the same SSH trust, 0600 on both ends.
    // Only with its forward actually up: without it a local listener could be
    // anything, and the app would send the token there. Without a token the
    // app simply keeps Send Later unavailable.
    if schedulerForwarded {
    let tokenQuery = ssh(connection, extra: [])
    tokenQuery.arguments! += ["cat \"$HOME/Library/Application Support/zapfast/scheduler-token\" 2>/dev/null || cat \"$HOME/.config/zapfast/scheduler-token\" 2>/dev/null || true"]
    let tokenOut = Pipe()
    tokenQuery.standardOutput = tokenOut
    if (try? tokenQuery.run()) != nil {
        let tokenTimeout = DispatchWorkItem { stop(tokenQuery) }
        DispatchQueue.global().asyncAfter(deadline: .now() + 10, execute: tokenTimeout)
        let tokenData = tokenOut.fileHandleForReading.readDataToEndOfFile()
        tokenTimeout.cancel()
        tokenQuery.waitUntilExit()
        if tokenQuery.terminationStatus == 0,
           let schedulerToken = String(data: tokenData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
           !schedulerToken.isEmpty, schedulerToken.count <= 256,
           valid(schedulerToken, pattern: "^[A-Za-z0-9._~+-]+$") {
            let configDir = profile.appendingPathComponent("config")
            try? manager.createDirectory(at: configDir, withIntermediateDirectories: true,
                                         attributes: [.posixPermissions: 0o700])
            try? schedulerToken.write(to: configDir.appendingPathComponent("scheduler-token"),
                                      atomically: true, encoding: .utf8)
            try? manager.setAttributes([.posixPermissions: 0o600],
                                       ofItemAtPath: configDir.appendingPathComponent("scheduler-token").path)
        }
    }
    }
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
