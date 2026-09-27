import AppKit
import Foundation
import UniformTypeIdentifiers

@MainActor
final class GlyphDocumentModel: ObservableObject {
    @Published var sourceURL: URL?
    @Published var sourceText = ""
    @Published var displayList: DisplayList?
    @Published var lastVisualDiff: VisualDiffReport?
    @Published var svg = ""
    @Published var selectedID: String?
    @Published var status = "Open a .glyph file"
    @Published var error: String?
    @Published var isDirty = false
    @Published var lastCheckOK = false
    @Published var canUndo = false
    @Published var canRedo = false
    @Published var lastManipulationPlan: DirectManipulationPlan?
    @Published var lastResizePlan: DirectResizePlan?
    @Published var lastHandleResizePlan: DirectHandleResizePlan?
    @Published var constraints: [ConstraintRecord] = []
    @Published var selectedConstraintID: String?

    private var lastKnownModificationDate: Date?
    private var lastSavedSource = ""
    private var undoStack: [String] = []
    private var redoStack: [String] = []
    private let historyLimit = 50

    var selectedItem: DisplayItem? {
        guard let id = selectedID else { return nil }
        return displayList?.items.first(where: { $0.id == id })
    }

    var selectedConstraints: [ConstraintRecord] {
        guard let id = selectedID else { return [] }
        return constraints.filter { $0.involves(id) }
    }

    func openPanel() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.init(filenameExtension: "glyph")!]
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url { open(url) }
    }

    func importImagePanel() {
        let sourcePanel = NSOpenPanel()
        sourcePanel.allowedContentTypes = [.image]
        sourcePanel.canChooseFiles = true
        sourcePanel.canChooseDirectories = false
        sourcePanel.allowsMultipleSelection = false
        guard sourcePanel.runModal() == .OK, let imageURL = sourcePanel.url else { return }

        let savePanel = NSSavePanel()
        savePanel.allowedContentTypes = [.init(filenameExtension: "glyph")!]
        savePanel.directoryURL = imageURL.deletingLastPathComponent()
        savePanel.nameFieldStringValue = imageURL.deletingPathExtension().lastPathComponent + ".glyph"
        guard savePanel.runModal() == .OK, let outputURL = savePanel.url else { return }
        importImage(imageURL, to: outputURL)
    }

    func importImage(_ imageURL: URL, to outputURL: URL) {
        let manifestURL = outputURL.deletingPathExtension().appendingPathExtension("vision.json")
        do {
            status = "Importing \(imageURL.lastPathComponent)…"
            _ = try runGlyph([
                "import-image", imageURL.path,
                "--output", outputURL.path,
                "--vision-helper", visionExecutable().path,
                "--manifest-output", manifestURL.path,
            ])
            open(outputURL)
            status = "Imported \(imageURL.lastPathComponent) · editable Glyph source"
        } catch {
            self.error = error.localizedDescription
            status = "Image import failed"
        }
    }

    func open(_ url: URL) {
        do {
            sourceURL = url
            sourceText = try String(contentsOf: url, encoding: .utf8)
            lastSavedSource = sourceText
            lastKnownModificationDate = modificationDate(url)
            isDirty = false
            selectedID = nil
            selectedConstraintID = nil
            constraints = []
            lastVisualDiff = nil
            lastManipulationPlan = nil
            lastResizePlan = nil
            lastHandleResizePlan = nil
            undoStack.removeAll()
            redoStack.removeAll()
            updateHistoryState()
            try refreshRuntime()
        } catch {
            self.error = error.localizedDescription
            status = "Could not open source"
        }
    }

    func markDirty() { isDirty = sourceText != lastSavedSource }

    func saveSource() {
        guard let url = sourceURL else { return }
        let candidate = url.deletingLastPathComponent()
            .appendingPathComponent(".glyphstudio-save-\(UUID().uuidString).glyph")
        defer { try? FileManager.default.removeItem(at: candidate) }
        do {
            try sourceText.write(to: candidate, atomically: true, encoding: .utf8)
            _ = try runGlyph(["check", candidate.path])
            if sourceText != lastSavedSource {
                pushUndo(lastSavedSource)
            }
            try sourceText.write(to: url, atomically: true, encoding: .utf8)
            lastSavedSource = sourceText
            lastKnownModificationDate = modificationDate(url)
            isDirty = false
            lastManipulationPlan = nil
            lastResizePlan = nil
            lastHandleResizePlan = nil
            try refreshRuntime()
        } catch {
            self.error = error.localizedDescription
            status = "Save rejected"
        }
    }

    func refresh() {
        do { try refreshRuntime() }
        catch {
            self.error = error.localizedDescription
            status = "Compile failed"
        }
    }

    func runAssertions() {
        guard let url = sourceURL else { return }
        do {
            let out = try runGlyph(["test", url.path])
            lastCheckOK = true
            status = "Assertions passed"
            error = nil
            _ = out
        } catch {
            lastCheckOK = false
            self.error = error.localizedDescription
            status = "Assertions failed"
        }
    }

    func setProperty(node: String, property: String, value: String) {
        guard let url = sourceURL, !property.isEmpty else { return }
        let directory = url.deletingLastPathComponent()
        let temporary = directory.appendingPathComponent(".glyphstudio-patch-\(UUID().uuidString).glyph")
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        do {
            let beforeSource = sourceText
            try beforeSource.write(to: beforeURL, atomically: true, encoding: .utf8)
            _ = try runGlyph(["source-set", beforeURL.path, node, property, value, "--output", temporary.path])
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(beforeSource)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            lastManipulationPlan = nil
            lastResizePlan = nil
            lastHandleResizePlan = nil
            selectedID = node
            status = "Patched \(node).\(property)"
        } catch {
            self.error = error.localizedDescription
            status = "Patch rejected"
        }
        try? FileManager.default.removeItem(at: temporary)
        try? FileManager.default.removeItem(at: beforeURL)
    }

    func editConstraint(
        sourceOrder: Int,
        relation: String? = nil,
        strength: String? = nil,
        offset: String? = nil
    ) {
        var options: [String] = []
        if let relation { options += ["--relation", relation] }
        if let strength { options += ["--strength", strength] }
        if let offset { options += ["--offset", offset] }
        guard !options.isEmpty else { return }
        mutateConstraintSource(
            statusLabel: "Edited constraint.\(sourceOrder)",
            selectedConstraintID: "constraint.\(sourceOrder)"
        ) { input, output in
            ["constraint-edit", input.path, String(sourceOrder)] + options + ["--output", output.path]
        }
    }

    func removeConstraint(sourceOrder: Int) {
        mutateConstraintSource(statusLabel: "Removed constraint.\(sourceOrder)", selectedConstraintID: nil) {
            input, output in
            ["constraint-remove", input.path, String(sourceOrder), "--output", output.path]
        }
    }

    func addConstraint(
        lhs: String,
        relation: String,
        rhs: String,
        offset: String? = nil,
        strength: String = "required"
    ) {
        var options = ["--strength", strength]
        if let offset, !offset.isEmpty { options += ["--offset", offset] }
        mutateConstraintSource(statusLabel: "Added constraint", selectedConstraintID: nil) {
            input, output in
            ["constraint-add", input.path, lhs, relation, rhs] + options + ["--output", output.path]
        }
    }

    func drag(node: String, dx: Double, dy: Double) {
        guard let url = sourceURL else { return }
        guard !isDirty else {
            status = "Save source edits before dragging"
            return
        }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-drag-\(UUID().uuidString).glyph")
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            let planText = try runGlyph(["drag-plan", beforeURL.path, node, String(dx), String(dy)])
            let plan = try JSONDecoder().decode(DirectManipulationPlan.self, from: Data(planText.utf8))
            lastManipulationPlan = plan
            lastResizePlan = nil
            lastHandleResizePlan = nil
            guard plan.allowed else {
                let ids = plan.blocked.flatMap(\.constraintIds).joined(separator: ", ")
                status = ids.isEmpty ? plan.note : "Drag blocked · \(ids)"
                try? FileManager.default.removeItem(at: beforeURL)
                return
            }
            _ = try runGlyph(["source-drag", beforeURL.path, node, String(dx), String(dy), "--output", temporary.path])
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            selectedID = node
            status = String(format: "Moved %@ · Δ %.1f, %.1f", node, dx, dy)
        } catch {
            self.error = error.localizedDescription
            status = "Drag rejected"
        }
        try? FileManager.default.removeItem(at: temporary)
        try? FileManager.default.removeItem(at: beforeURL)
    }

    func resize(node: String, dw: Double, dh: Double) {
        guard let url = sourceURL else { return }
        guard !isDirty else {
            status = "Save source edits before resizing"
            return
        }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-resize-\(UUID().uuidString).glyph")
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            let planText = try runGlyph(["resize-plan", beforeURL.path, node, String(dw), String(dh)])
            let plan = try JSONDecoder().decode(DirectResizePlan.self, from: Data(planText.utf8))
            lastResizePlan = plan
            lastHandleResizePlan = nil
            lastManipulationPlan = nil
            guard plan.allowed else {
                let ids = plan.blocked.flatMap(\.constraintIds).joined(separator: ", ")
                status = ids.isEmpty ? plan.note : "Resize blocked · \(ids)"
                try? FileManager.default.removeItem(at: beforeURL)
                return
            }
            _ = try runGlyph(["source-resize", beforeURL.path, node, String(dw), String(dh), "--output", temporary.path])
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            selectedID = node
            status = String(format: "Resized %@ · Δ %.1f × %.1f", node, dw, dh)
        } catch {
            self.error = error.localizedDescription
            status = "Resize rejected"
        }
        try? FileManager.default.removeItem(at: temporary)
        try? FileManager.default.removeItem(at: beforeURL)
    }

    func resize(node: String, handle: String, dx: Double, dy: Double) {
        guard let url = sourceURL else { return }
        guard !isDirty else {
            status = "Save source edits before resizing"
            return
        }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-resize-handle-\(UUID().uuidString).glyph")
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            let planText = try runGlyph([
                "resize-handle-plan", beforeURL.path, node, handle, String(dx), String(dy),
            ])
            let plan = try JSONDecoder().decode(DirectHandleResizePlan.self, from: Data(planText.utf8))
            lastHandleResizePlan = plan
            lastResizePlan = plan.resizePlan
            lastManipulationPlan = plan.movementPlan
            guard plan.allowed else {
                let ids = plan.blocked.flatMap(\.constraintIds).joined(separator: ", ")
                status = ids.isEmpty ? plan.note : "Resize blocked · \(ids)"
                try? FileManager.default.removeItem(at: beforeURL)
                return
            }
            _ = try runGlyph([
                "source-resize-handle", beforeURL.path, node, handle, String(dx), String(dy),
                "--output", temporary.path,
            ])
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            selectedID = node
            status = String(format: "Resized %@ from %@ · Δ %.1f, %.1f", node, handle, dx, dy)
        } catch {
            self.error = error.localizedDescription
            status = "Resize rejected"
        }
        try? FileManager.default.removeItem(at: temporary)
        try? FileManager.default.removeItem(at: beforeURL)
    }

    func undoSemanticEdit() {
        guard !isDirty, let url = sourceURL, let previous = undoStack.popLast() else { return }
        let current = sourceText
        redoStack.append(current)
        do {
            try restoreHistorySource(previous, at: url, label: "Undo")
        } catch {
            redoStack.removeLast()
            undoStack.append(previous)
            self.error = error.localizedDescription
        }
        updateHistoryState()
    }

    func redoSemanticEdit() {
        guard !isDirty, let url = sourceURL, let next = redoStack.popLast() else { return }
        let current = sourceText
        undoStack.append(current)
        do {
            try restoreHistorySource(next, at: url, label: "Redo")
        } catch {
            undoStack.removeLast()
            redoStack.append(next)
            self.error = error.localizedDescription
        }
        updateHistoryState()
    }

    func pollExternalChange() {
        guard let url = sourceURL, !isDirty, let current = modificationDate(url) else { return }
        if let known = lastKnownModificationDate, current > known {
            open(url)
        }
    }

    private func mutateConstraintSource(
        statusLabel: String,
        selectedConstraintID: String?,
        arguments: (URL, URL) -> [String]
    ) {
        guard let url = sourceURL else { return }
        guard !isDirty else {
            status = "Save source edits before editing constraints"
            return
        }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-constraint-\(UUID().uuidString).glyph")
        defer {
            try? FileManager.default.removeItem(at: temporary)
            try? FileManager.default.removeItem(at: beforeURL)
        }
        do {
            let beforeSource = sourceText
            try beforeSource.write(to: beforeURL, atomically: true, encoding: .utf8)
            _ = try runGlyph(arguments(beforeURL, temporary))
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(beforeSource)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            self.selectedConstraintID = selectedConstraintID
            lastManipulationPlan = nil
            lastResizePlan = nil
            lastHandleResizePlan = nil
            status = statusLabel
        } catch {
            self.error = error.localizedDescription
            status = "Constraint edit rejected"
        }
    }

    private func pushUndo(_ source: String) {
        if undoStack.last != source {
            undoStack.append(source)
            if undoStack.count > historyLimit {
                undoStack.removeFirst(undoStack.count - historyLimit)
            }
        }
        redoStack.removeAll()
        updateHistoryState()
    }

    private func updateHistoryState() {
        canUndo = !undoStack.isEmpty
        canRedo = !redoStack.isEmpty
    }

    private func commitSource(_ replacement: String, to url: URL) throws {
        try replacement.write(to: url, atomically: true, encoding: .utf8)
        sourceText = replacement
        lastSavedSource = replacement
        lastKnownModificationDate = modificationDate(url)
        isDirty = false
        try refreshRuntime()
    }

    private func updateVisualDiff(beforeURL: URL, afterURL: URL) {
        if let diffText = try? runGlyph(["visual-diff", beforeURL.path, afterURL.path]),
           let diffData = diffText.data(using: .utf8) {
            lastVisualDiff = try? JSONDecoder().decode(VisualDiffReport.self, from: diffData)
        } else {
            lastVisualDiff = nil
        }
    }

    private func restoreHistorySource(_ replacement: String, at url: URL, label: String) throws {
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-history-before-\(UUID().uuidString).glyph")
        let candidateURL = directory.appendingPathComponent(".glyphstudio-history-candidate-\(UUID().uuidString).glyph")
        defer {
            try? FileManager.default.removeItem(at: beforeURL)
            try? FileManager.default.removeItem(at: candidateURL)
        }
        try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
        try replacement.write(to: candidateURL, atomically: true, encoding: .utf8)
        _ = try runGlyph(["check", candidateURL.path])
        try commitSource(replacement, to: url)
        updateVisualDiff(beforeURL: beforeURL, afterURL: url)
        lastManipulationPlan = nil
        lastResizePlan = nil
            lastHandleResizePlan = nil
        status = label
    }

    private func refreshRuntime() throws {
        guard let url = sourceURL else { return }
        _ = try runGlyph(["check", url.path])
        let dlText = try runGlyph(["display-list", url.path])
        let data = Data(dlText.utf8)
        displayList = try JSONDecoder().decode(DisplayList.self, from: data)
        let constraintText = try runGlyph(["constraints", url.path])
        constraints = try JSONDecoder().decode([ConstraintRecord].self, from: Data(constraintText.utf8))
        if let selectedConstraintID,
           !constraints.contains(where: { $0.id == selectedConstraintID }) {
            self.selectedConstraintID = nil
        }

        let svgURL = FileManager.default.temporaryDirectory.appendingPathComponent("glyph-studio-\(UUID().uuidString).svg")
        _ = try runGlyph(["render", url.path, "--output", svgURL.path])
        svg = try String(contentsOf: svgURL, encoding: .utf8)
        try? FileManager.default.removeItem(at: svgURL)
        lastCheckOK = true
        error = nil
        status = "\(displayList?.items.count ?? 0) objects · revision \(displayList?.revision ?? 0)"
    }

    private func runGlyph(_ arguments: [String]) throws -> String {
        let process = Process()
        process.executableURL = glyphExecutable()
        process.arguments = arguments
        let stdout = Pipe(); let stderr = Pipe()
        process.standardOutput = stdout; process.standardError = stderr
        try process.run(); process.waitUntilExit()
        let out = String(data: stdout.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let err = String(data: stderr.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        guard process.terminationStatus == 0 else {
            throw NSError(domain: "GlyphStudio", code: Int(process.terminationStatus), userInfo: [NSLocalizedDescriptionKey: err.isEmpty ? out : err])
        }
        return out
    }

    private func glyphExecutable() -> URL {
        if let explicit = ProcessInfo.processInfo.environment["GLYPH_BIN"] {
            return URL(fileURLWithPath: explicit)
        }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let candidates = [
            cwd.appendingPathComponent("target/release/glyph"),
            cwd.deletingLastPathComponent().appendingPathComponent("target/release/glyph"),
            URL(fileURLWithPath: "/Users/joshuakhoury/Projects/Glyph/target/release/glyph"),
        ]
        return candidates.first(where: { FileManager.default.isExecutableFile(atPath: $0.path) }) ?? candidates.last!
    }

    private func visionExecutable() -> URL {
        if let explicit = ProcessInfo.processInfo.environment["GLYPH_VISION_BIN"] {
            return URL(fileURLWithPath: explicit)
        }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let candidates = [
            cwd.appendingPathComponent("studio/.build/release/GlyphVision"),
            cwd.appendingPathComponent(".build/release/GlyphVision"),
            cwd.deletingLastPathComponent().appendingPathComponent("studio/.build/release/GlyphVision"),
            URL(fileURLWithPath: "/Users/joshuakhoury/Projects/Glyph/studio/.build/release/GlyphVision"),
        ]
        return candidates.first(where: { FileManager.default.isExecutableFile(atPath: $0.path) }) ?? candidates.last!
    }

    private func modificationDate(_ url: URL) -> Date? {
        try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
    }
}
