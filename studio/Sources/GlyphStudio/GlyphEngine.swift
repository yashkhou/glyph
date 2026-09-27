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
    @Published var selectedIDs: Set<String> = []
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
    @Published var sourceHierarchy: [DirectSourceNode] = []
    @Published var components: [ComponentCatalogEntry] = []
    @Published var selectedConstraintID: String?
    @Published var solverReused = false
    @Published var solverRebuildCount = 0

    private var lastKnownModificationDate: Date?
    private var lastSavedSource = ""
    private var undoStack: [String] = []
    private var redoStack: [String] = []
    private let historyLimit = 50
    private var runtimeSessionProcess: Process?
    private var runtimeSessionInput: FileHandle?
    private var runtimeSessionOutput: FileHandle?
    private var runtimeSessionError: FileHandle?
    private var runtimeSessionURL: URL?
    private var runtimeSessionBuffer = Data()

    var selectedItem: DisplayItem? {
        guard let id = selectedID else { return nil }
        return displayList?.items.first(where: { $0.id == id })
    }

    var selectedConstraints: [ConstraintRecord] {
        guard let id = selectedID else { return [] }
        return constraints.filter { $0.involves(id) }
    }

    var canGroupSelection: Bool { selectedIDs.count >= 2 && !isDirty }

    var canExtractComponentSelection: Bool {
        guard !isDirty, !selectedIDs.isEmpty else { return false }
        return selectedIDs.allSatisfy { id in
            sourceHierarchy.contains { node in
                node.id == id && node.kind != "group" && !node.kind.hasPrefix("instance:")
            }
        }
    }

    var canUngroupSelection: Bool {
        !isDirty && selectedIDs.count == 1 && selectedItem?.primitive.kind == "group"
    }

    var selectedHierarchyNode: DirectSourceNode? {
        guard !isDirty, selectedIDs.count == 1, let selectedID else { return nil }
        return sourceHierarchy.first { $0.id == selectedID && !$0.kind.hasPrefix("instance:") }
    }

    var availableHierarchyParents: [DirectSourceNode] {
        guard let selected = selectedHierarchyNode else { return [] }
        return sourceHierarchy.filter { $0.kind == "group" && $0.id != selected.id }
    }

    private var paintHierarchy: [DirectSourceNode] {
        sourceHierarchy.filter { $0.kind != "group" }
    }

    private var selectedSourceOwner: DirectSourceNode? {
        guard !isDirty, selectedIDs.count == 1, let selectedID else { return nil }
        if let exact = paintHierarchy.first(where: { $0.id == selectedID }) {
            return exact
        }
        return paintHierarchy
            .filter { $0.kind.hasPrefix("instance:") && selectedID.hasPrefix($0.id + "/") }
            .max { $0.id.count < $1.id.count }
    }

    private var selectedSourceIndex: Int? {
        guard let owner = selectedSourceOwner else { return nil }
        return paintHierarchy.firstIndex(where: { $0.id == owner.id })
    }

    var selectedInstanceOwnerID: String? {
        guard let owner = selectedSourceOwner, owner.kind.hasPrefix("instance:") else { return nil }
        return owner.id
    }

    var selectedInstanceComponentName: String? {
        guard let owner = selectedSourceOwner, owner.kind.hasPrefix("instance:") else { return nil }
        return String(owner.kind.dropFirst("instance:".count))
    }

    var canRemoveSelectedInstance: Bool {
        selectedInstanceOwnerID != nil && !isDirty
    }

    var canSendBackward: Bool {
        guard let index = selectedSourceIndex else { return false }
        return index > 0
    }

    var canBringForward: Bool {
        guard let index = selectedSourceIndex else { return false }
        return index + 1 < paintHierarchy.count
    }

    func select(_ id: String, extending: Bool = false) {
        if extending {
            if selectedIDs.contains(id) {
                selectedIDs.remove(id)
                if selectedID == id {
                    selectedID = selectedIDs.first
                }
            } else {
                selectedIDs.insert(id)
                selectedID = id
            }
        } else {
            selectedIDs = [id]
            selectedID = id
        }
        selectedConstraintID = nil
    }

    func clearSelection() {
        selectedID = nil
        selectedIDs.removeAll()
        selectedConstraintID = nil
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
            stopRuntimeSession()
            sourceURL = url
            sourceText = try String(contentsOf: url, encoding: .utf8)
            lastSavedSource = sourceText
            lastKnownModificationDate = modificationDate(url)
            isDirty = false
            selectedID = nil
            selectedIDs.removeAll()
            selectedConstraintID = nil
            constraints = []
            sourceHierarchy = []
            components = []
            lastVisualDiff = nil
            lastManipulationPlan = nil
            lastResizePlan = nil
            lastHandleResizePlan = nil
            solverReused = false
            solverRebuildCount = 0
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
            select(node)
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

    func addComponentInstance(
        component: String,
        name: String,
        x: String = "",
        y: String = "",
        scale: String = "",
        parameters: [String: String] = [:],
        slots: [String: String] = [:],
        variant: String? = nil
    ) {
        var options: [String] = []
        if !x.isEmpty { options += ["--x", x] }
        if !y.isEmpty { options += ["--y", y] }
        if !scale.isEmpty { options += ["--scale", scale] }
        for key in parameters.keys.sorted() {
            if let value = parameters[key], !value.isEmpty { options += ["--param", "\(key)=\(value)"] }
        }
        for key in slots.keys.sorted() {
            if let value = slots[key], !value.isEmpty { options += ["--slot", "\(key)=\(value)"] }
        }
        if let variant, !variant.isEmpty { options += ["--variant", variant] }
        mutateComponentSource(statusLabel: "Added component instance \(name)", selectionAfter: name) {
            input, output in
            ["instance-add", input.path, component, name] + options + ["--output", output.path]
        }
    }

    func removeSelectedInstance() {
        guard let instance = selectedInstanceOwnerID else { return }
        mutateComponentSource(statusLabel: "Removed component instance \(instance)", selectionAfter: nil) {
            input, output in
            ["instance-remove", input.path, instance, "--output", output.path]
        }
    }

    func setInstanceParameter(instance: String, parameter: String, value: String) {
        guard !parameter.isEmpty, !value.isEmpty else { return }
        let keepSelection = selectedID
        mutateComponentSource(statusLabel: "Updated \(instance).\(parameter)", selectionAfter: keepSelection) {
            input, output in
            ["instance-param-set", input.path, instance, parameter, value, "--output", output.path]
        }
    }

    func setInstanceSlot(instance: String, slot: String, value: String) {
        guard !slot.isEmpty, !value.isEmpty else { return }
        let keepSelection = selectedID
        mutateComponentSource(statusLabel: "Updated \(instance).\(slot)", selectionAfter: keepSelection) {
            input, output in
            ["instance-slot-set", input.path, instance, slot, value, "--output", output.path]
        }
    }

    func extractSelectionAsComponent(component: String, instance: String) {
        guard canExtractComponentSelection, !component.isEmpty, !instance.isEmpty else { return }
        let ordered = displayList?.items
            .filter { selectedIDs.contains($0.id) }
            .map(\.id) ?? Array(selectedIDs).sorted()
        guard !ordered.isEmpty else { return }
        mutateComponentSource(
            statusLabel: "Extracted \(ordered.count) objects into \(component)",
            selectionAfter: instance
        ) { input, output in
            ["component-extract", input.path, component, instance] + ordered + ["--output", output.path]
        }
    }

    func groupSelection() {
        guard let url = sourceURL, canGroupSelection else { return }
        let ordered = displayList?.items
            .filter { selectedIDs.contains($0.id) }
            .map(\.id) ?? Array(selectedIDs).sorted()
        guard ordered.count >= 2 else { return }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-group-\(UUID().uuidString).glyph")
        defer {
            try? FileManager.default.removeItem(at: temporary)
            try? FileManager.default.removeItem(at: beforeURL)
        }
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            let resultText = try runGlyph(["group-create", beforeURL.path] + ordered + ["--output", temporary.path])
            _ = try runGlyph(["check", temporary.path])
            let result = try JSONSerialization.jsonObject(with: Data(resultText.utf8)) as? [String: Any]
            let groupID = result?["group"] as? String
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            if let groupID {
                select(groupID)
            } else {
                clearSelection()
            }
            status = "Grouped \(ordered.count) objects"
        } catch {
            self.error = error.localizedDescription
            status = "Grouping rejected"
        }
    }

    func ungroupSelected() {
        guard let url = sourceURL, canUngroupSelection, let groupID = selectedID else { return }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-ungroup-\(UUID().uuidString).glyph")
        defer {
            try? FileManager.default.removeItem(at: temporary)
            try? FileManager.default.removeItem(at: beforeURL)
        }
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            _ = try runGlyph(["group-remove", beforeURL.path, groupID, "--output", temporary.path])
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            clearSelection()
            status = "Ungrouped \(groupID)"
        } catch {
            self.error = error.localizedDescription
            status = "Ungroup rejected"
        }
    }

    func reparentSelected(to parent: String?) {
        guard let url = sourceURL, let selected = selectedHierarchyNode else { return }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-reparent-\(UUID().uuidString).glyph")
        defer {
            try? FileManager.default.removeItem(at: temporary)
            try? FileManager.default.removeItem(at: beforeURL)
        }
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            var arguments = ["node-reparent", beforeURL.path, selected.id]
            if let parent { arguments += ["--parent", parent] }
            arguments += ["--output", temporary.path]
            _ = try runGlyph(arguments)
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            select(selected.id)
            status = parent.map { "Moved \(selected.id) into \($0)" } ?? "Moved \(selected.id) to scene root"
        } catch {
            self.error = error.localizedDescription
            status = "Reparent rejected"
        }
    }

    func sendSelectedBackward() {
        guard let index = selectedSourceIndex, index > 0, let node = selectedID else { return }
        let anchor = paintHierarchy[index - 1].id
        moveSourceNode(node: node, anchor: anchor, placement: "before", label: "Sent \(node) backward")
    }

    func bringSelectedForward() {
        guard let index = selectedSourceIndex,
              index + 1 < paintHierarchy.count,
              let node = selectedID else { return }
        let anchor = paintHierarchy[index + 1].id
        moveSourceNode(node: node, anchor: anchor, placement: "after", label: "Brought \(node) forward")
    }

    private func moveSourceNode(node: String, anchor: String, placement: String, label: String) {
        guard let url = sourceURL, !isDirty else {
            status = "Save source edits before reordering"
            return
        }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-reorder-\(UUID().uuidString).glyph")
        defer {
            try? FileManager.default.removeItem(at: temporary)
            try? FileManager.default.removeItem(at: beforeURL)
        }
        do {
            try sourceText.write(to: beforeURL, atomically: true, encoding: .utf8)
            _ = try runGlyph([
                "node-move", beforeURL.path, node, anchor, placement,
                "--output", temporary.path,
            ])
            _ = try runGlyph(["check", temporary.path])
            let replacement = try String(contentsOf: temporary, encoding: .utf8)
            pushUndo(sourceText)
            try commitSource(replacement, to: url)
            updateVisualDiff(beforeURL: beforeURL, afterURL: url)
            select(node)
            status = label
        } catch {
            self.error = error.localizedDescription
            status = "Reorder rejected"
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
            select(node)
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

    private func mutateComponentSource(
        statusLabel: String,
        selectionAfter: String?,
        arguments: (URL, URL) -> [String]
    ) {
        guard let url = sourceURL else { return }
        guard !isDirty else {
            status = "Save source edits before editing components"
            return
        }
        let directory = url.deletingLastPathComponent()
        let beforeURL = directory.appendingPathComponent(".glyphstudio-before-\(UUID().uuidString).glyph")
        let temporary = directory.appendingPathComponent(".glyphstudio-component-\(UUID().uuidString).glyph")
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
            lastManipulationPlan = nil
            lastResizePlan = nil
            lastHandleResizePlan = nil
            if let selectionAfter {
                let resolved = displayList?.items.first(where: {
                    $0.id == selectionAfter || $0.id.hasPrefix(selectionAfter + "/")
                })?.id
                if let resolved { select(resolved) } else { clearSelection() }
            } else {
                clearSelection()
            }
            status = statusLabel
        } catch {
            self.error = error.localizedDescription
            status = "Component edit rejected"
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

    private func stopRuntimeSession() {
        if runtimeSessionProcess?.isRunning == true, let input = runtimeSessionInput {
            input.write(Data("{\"op\":\"quit\"}\n".utf8))
        }
        try? runtimeSessionInput?.close()
        try? runtimeSessionOutput?.close()
        try? runtimeSessionError?.close()
        runtimeSessionInput = nil
        runtimeSessionOutput = nil
        runtimeSessionError = nil
        runtimeSessionProcess = nil
        runtimeSessionURL = nil
        runtimeSessionBuffer.removeAll(keepingCapacity: false)
    }

    private func readRuntimeSessionLine() throws -> Data {
        guard let output = runtimeSessionOutput else {
            throw NSError(
                domain: "GlyphStudio",
                code: 90,
                userInfo: [NSLocalizedDescriptionKey: "Glyph Studio runtime session has no output stream"]
            )
        }
        while true {
            if let newline = runtimeSessionBuffer.firstIndex(of: 0x0A) {
                let line = Data(runtimeSessionBuffer[..<newline])
                let after = runtimeSessionBuffer.index(after: newline)
                runtimeSessionBuffer.removeSubrange(runtimeSessionBuffer.startIndex..<after)
                if !line.isEmpty { return line }
                continue
            }
            let chunk = output.availableData
            if chunk.isEmpty {
                let stderr = runtimeSessionError?.readDataToEndOfFile() ?? Data()
                let message = String(data: stderr, encoding: .utf8) ?? ""
                stopRuntimeSession()
                throw NSError(
                    domain: "GlyphStudio",
                    code: 91,
                    userInfo: [
                        NSLocalizedDescriptionKey: message.isEmpty
                            ? "Glyph Studio runtime session ended unexpectedly"
                            : message
                    ]
                )
            }
            runtimeSessionBuffer.append(chunk)
        }
    }

    private func ensureRuntimeSession(for url: URL) throws {
        let canonicalURL = url.standardizedFileURL
        if runtimeSessionURL == canonicalURL, runtimeSessionProcess?.isRunning == true { return }
        stopRuntimeSession()

        let process = Process()
        process.executableURL = glyphExecutable()
        process.arguments = ["studio-session", canonicalURL.path]
        let input = Pipe()
        let output = Pipe()
        let error = Pipe()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = error
        try process.run()

        runtimeSessionProcess = process
        runtimeSessionInput = input.fileHandleForWriting
        runtimeSessionOutput = output.fileHandleForReading
        runtimeSessionError = error.fileHandleForReading
        runtimeSessionURL = canonicalURL
        runtimeSessionBuffer.removeAll(keepingCapacity: true)

        do {
            let ready = try JSONDecoder().decode(StudioSessionResponse.self, from: readRuntimeSessionLine())
            guard ready.ok, ready.event == "ready" else {
                throw NSError(
                    domain: "GlyphStudio",
                    code: 92,
                    userInfo: [NSLocalizedDescriptionKey: ready.error ?? "Glyph Studio runtime session did not become ready"]
                )
            }
        } catch {
            stopRuntimeSession()
            throw error
        }
    }

    private func runtimeSessionRefresh(_ url: URL) throws -> StudioSessionResponse {
        try ensureRuntimeSession(for: url)
        guard let input = runtimeSessionInput else {
            throw NSError(
                domain: "GlyphStudio",
                code: 93,
                userInfo: [NSLocalizedDescriptionKey: "Glyph Studio runtime session has no input stream"]
            )
        }
        input.write(Data("{\"op\":\"refresh\"}\n".utf8))
        let response = try JSONDecoder().decode(StudioSessionResponse.self, from: readRuntimeSessionLine())
        guard response.ok else {
            throw NSError(
                domain: "GlyphStudio",
                code: 94,
                userInfo: [NSLocalizedDescriptionKey: response.error ?? "Glyph Studio runtime refresh failed"]
            )
        }
        return response
    }

    private func refreshRuntime() throws {
        guard let url = sourceURL else { return }
        let response = try runtimeSessionRefresh(url)
        guard let nextDisplayList = response.displayList,
              let nextHierarchy = response.hierarchy,
              let nextComponents = response.components,
              let nextConstraints = response.constraints,
              let nextSVG = response.svg else {
            throw NSError(
                domain: "GlyphStudio",
                code: 95,
                userInfo: [NSLocalizedDescriptionKey: "Glyph Studio runtime refresh returned incomplete scene data"]
            )
        }

        displayList = nextDisplayList
        sourceHierarchy = nextHierarchy.nodes
        components = nextComponents
        constraints = nextConstraints
        svg = nextSVG
        if let selectedConstraintID,
           !constraints.contains(where: { $0.id == selectedConstraintID }) {
            self.selectedConstraintID = nil
        }

        solverReused = response.session?.reusedSolver ?? false
        solverRebuildCount = response.session?.rebuildCount ?? solverRebuildCount
        lastCheckOK = true
        error = nil
        let solverMode = solverReused ? "incremental" : "rebuilt"
        status = "\(displayList?.items.count ?? 0) objects · revision \(displayList?.revision ?? 0) · solver \(solverMode)"
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
