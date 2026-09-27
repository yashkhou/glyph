import AppKit
import Combine
import SwiftUI

private struct ResizeHandleSpec: Identifiable {
    let id: String
    let xFactor: Double
    let yFactor: Double
    let horizontal: Bool
    let vertical: Bool
}

struct ContentView: View {
    @StateObject var model: GlyphDocumentModel
    @State private var patchProperty = ""
    @State private var patchValue = ""
    @State private var resizePreview = CGSize.zero
    @State private var activeResizeHandle: String?
    @State private var groupDragPreview = CGSize.zero
    @State private var newConstraintLHS = ""
    @State private var newConstraintRHS = ""
    @State private var newConstraintOffset = ""
    @State private var newConstraintRelation = "eq"
    @State private var newConstraintStrength = "required"
    @State private var showComponentLibrary = false
    @State private var showComponentExtraction = false
    @State private var extractedComponentName = ""
    @State private var extractedInstanceName = ""
    @State private var componentSelection = ""
    @State private var instanceName = ""
    @State private var instanceX = ""
    @State private var instanceY = ""
    @State private var instanceScale = ""
    @State private var instanceVariant = ""
    @State private var componentBindingValues: [String: String] = [:]
    @State private var selectedInstanceParameterValues: [String: String] = [:]
    private let poll = Timer.publish(every: 1.0, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            commandBar
            Divider().opacity(0.45)
            HSplitView {
                objectLedger
                    .frame(minWidth: 210, idealWidth: 245, maxWidth: 300)
                VSplitView {
                    previewPlate
                        .frame(minHeight: 380)
                    sourceBench
                        .frame(minHeight: 170, idealHeight: 240)
                }
                intentInspector
                    .frame(minWidth: 250, idealWidth: 285, maxWidth: 330)
            }
            Divider().opacity(0.45)
            statusBar
        }
        .background(Color(nsColor: .windowBackgroundColor).opacity(0.98))
        .frame(minWidth: 1040, minHeight: 720)
        .onReceive(poll) { _ in model.pollExternalChange() }
        .alert("Glyph", isPresented: Binding(get: { model.error != nil }, set: { if !$0 { model.error = nil } })) {
            Button("Dismiss", role: .cancel) { model.error = nil }
        } message: { Text(model.error ?? "") }
        .sheet(isPresented: $showComponentLibrary) {
            componentLibrarySheet
                .frame(minWidth: 520, idealWidth: 600, minHeight: 520, idealHeight: 620)
        }
        .sheet(isPresented: $showComponentExtraction) {
            componentExtractionSheet
                .frame(minWidth: 430, idealWidth: 470, minHeight: 250, idealHeight: 280)
        }
    }

    private var commandBar: some View {
        HStack(spacing: 10) {
            Text("GLYPH")
                .font(.system(size: 13, weight: .black, design: .rounded))
                .tracking(2.4)
            Text("STUDIO")
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(.secondary)
            Rectangle().fill(.quaternary).frame(width: 1, height: 20).padding(.horizontal, 4)
            Button("Open") { model.openPanel() }
            Button("Import Image…") { model.importImagePanel() }
            Button("Refresh") { model.refresh() }
                .disabled(model.sourceURL == nil)
            Button("Undo") { model.undoSemanticEdit() }
                .disabled(!model.canUndo || model.isDirty)
            Button("Redo") { model.redoSemanticEdit() }
                .disabled(!model.canRedo || model.isDirty)
            Button("Group") { model.groupSelection() }
                .disabled(!model.canGroupSelection)
                .help("Group the selected authored objects (⌘-click in the object ledger to multi-select)")
            Button("Ungroup") { model.ungroupSelected() }
                .disabled(!model.canUngroupSelection)
            Button("Make Component…") {
                if extractedComponentName.isEmpty { extractedComponentName = "component" }
                if extractedInstanceName.isEmpty { extractedInstanceName = "component1" }
                showComponentExtraction = true
            }
            .disabled(!model.canExtractComponentSelection)
            .help("Extract the selected contiguous authored layers into a reusable local component")
            Button("Components…") {
                if componentSelection.isEmpty { componentSelection = model.components.first?.name ?? "" }
                showComponentLibrary = true
            }
            .disabled(model.components.isEmpty || model.isDirty)
            Button("Remove Instance") { model.removeSelectedInstance() }
                .disabled(!model.canRemoveSelectedInstance)
                .help("Remove the selected component-instance owner from authored source")
            Button("Send Backward") { model.sendSelectedBackward() }
                .disabled(!model.canSendBackward)
                .help("Move the selected directly-authored node one step earlier in source/render order")
            Button("Bring Forward") { model.bringSelectedForward() }
                .disabled(!model.canBringForward)
                .help("Move the selected directly-authored node one step later in source/render order")
            Button(model.isDirty ? "Save source •" : "Save source") { model.saveSource() }
                .disabled(model.sourceURL == nil || !model.isDirty)
            Button("Run assertions") { model.runAssertions() }
                .disabled(model.sourceURL == nil)
            Spacer()
            if let scene = model.displayList {
                Text("\(Int(scene.canvas.width)) × \(Int(scene.canvas.height))")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 14)
        .frame(height: 42)
    }

    private var objectLedger: some View {
        VStack(alignment: .leading, spacing: 0) {
            railHeader("OBJECT LEDGER", trailing: "\(model.displayList?.items.count ?? 0)")
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(model.displayList?.items ?? []) { item in
                        Button {
                            let extending = NSEvent.modifierFlags.contains(.command)
                                || NSEvent.modifierFlags.contains(.shift)
                            model.select(item.id, extending: extending)
                            patchProperty = ""
                            patchValue = ""
                        } label: {
                            HStack(spacing: 9) {
                                Rectangle()
                                    .fill(model.selectedIDs.contains(item.id) ? Color.accentColor : Color.secondary.opacity(0.22))
                                    .frame(width: 2, height: 30)
                                VStack(alignment: .leading, spacing: 3) {
                                    HStack(spacing: 5) {
                                        Text(item.name)
                                            .font(.system(size: 12, weight: .semibold))
                                            .lineLimit(1)
                                        Spacer(minLength: 4)
                                        Text(item.primitive.kind.uppercased())
                                            .font(.system(size: 8, weight: .bold, design: .monospaced))
                                            .foregroundStyle(.tertiary)
                                    }
                                    Text(item.roles.joined(separator: " · "))
                                        .font(.system(size: 9, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                        .lineLimit(1)
                                }
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 7)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        Divider().padding(.leading, 21).opacity(0.28)
                    }
                }
            }
        }
        .background(Color(nsColor: .underPageBackgroundColor).opacity(0.55))
    }

    private var previewPlate: some View {
        VStack(spacing: 0) {
            railHeader("RESOLVED PLATE", trailing: model.selectedID ?? "NO SELECTION")
            ZStack {
                Color(nsColor: .controlBackgroundColor).opacity(0.35)
                if model.svg.isEmpty {
                    VStack(spacing: 8) {
                        Image(systemName: "viewfinder")
                            .font(.system(size: 28, weight: .light))
                            .foregroundStyle(.tertiary)
                        Text("No resolved scene")
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                } else {
                    SVGPreview(
                        svg: model.svg,
                        selectedID: model.selectedID,
                        onSelect: { id in
                            model.select(id)
                            patchProperty = ""
                            patchValue = ""
                            resizePreview = .zero
                            activeResizeHandle = nil
                            groupDragPreview = .zero
                        },
                        onDrag: { id, dx, dy in
                            model.drag(node: id, dx: dx, dy: dy)
                        }
                    )
                    .padding(22)
                    .overlay { semanticConstraintOverlay }
                    .overlay { semanticGroupDragOverlay }
                    .overlay { semanticResizeOverlay }
                }
            }
        }
    }

    @ViewBuilder
    private var semanticConstraintOverlay: some View {
        GeometryReader { proxy in
            if let scene = model.displayList,
               let item = model.selectedItem,
               let bounds = item.bounds,
               bounds.count == 4,
               !model.selectedConstraints.isEmpty {
                let innerWidth = max(proxy.size.width - 44, 1)
                let innerHeight = max(proxy.size.height - 44, 1)
                let scale = min(innerWidth / CGFloat(scene.canvas.width), innerHeight / CGFloat(scene.canvas.height))
                let renderedWidth = CGFloat(scene.canvas.width) * scale
                let renderedHeight = CGFloat(scene.canvas.height) * scale
                let originX = 22 + (innerWidth - renderedWidth) / 2
                let originY = 22 + (innerHeight - renderedHeight) / 2
                HStack(spacing: 4) {
                    ForEach(model.selectedConstraints) { constraint in
                        Button {
                            model.selectedConstraintID = constraint.id
                        } label: {
                            Text("C\(constraint.sourceOrder)")
                                .font(.system(size: 9, weight: .bold, design: .monospaced))
                                .padding(.horizontal, 6).padding(.vertical, 3)
                                .background(.regularMaterial, in: Capsule())
                                .overlay(Capsule().stroke(model.selectedConstraintID == constraint.id ? Color.accentColor : .secondary.opacity(0.45), lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Constraint \(constraint.sourceOrder): \(constraint.displayExpression)")
                    }
                }
                .position(
                    x: originX + CGFloat(bounds[0]) * scale + 18,
                    y: max(12, originY + CGFloat(bounds[1]) * scale - 14)
                )
            }
        }
    }

    @ViewBuilder
    private var semanticGroupDragOverlay: some View {
        GeometryReader { proxy in
            if let scene = model.displayList,
               let item = model.selectedItem,
               item.primitive.kind == "group",
               let bounds = item.bounds,
               bounds.count == 4 {
                let innerWidth = max(proxy.size.width - 44, 1)
                let innerHeight = max(proxy.size.height - 44, 1)
                let scale = min(innerWidth / CGFloat(scene.canvas.width), innerHeight / CGFloat(scene.canvas.height))
                let renderedWidth = CGFloat(scene.canvas.width) * scale
                let renderedHeight = CGFloat(scene.canvas.height) * scale
                let originX = 22 + (innerWidth - renderedWidth) / 2
                let originY = 22 + (innerHeight - renderedHeight) / 2
                let width = max(CGFloat(bounds[2]) * scale, 14)
                let height = max(CGFloat(bounds[3]) * scale, 14)
                let centerX = originX + CGFloat(bounds[0] + bounds[2] / 2) * scale
                let centerY = originY + CGFloat(bounds[1] + bounds[3] / 2) * scale

                Rectangle()
                    .fill(Color.clear)
                    .contentShape(Rectangle())
                    .overlay(Rectangle().stroke(Color.accentColor.opacity(0.8), style: StrokeStyle(lineWidth: 1, dash: [5, 4])))
                    .frame(width: width, height: height)
                    .position(x: centerX + groupDragPreview.width, y: centerY + groupDragPreview.height)
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { value in groupDragPreview = value.translation }
                            .onEnded { value in
                                groupDragPreview = .zero
                                guard scale > 0 else { return }
                                model.drag(
                                    node: item.id,
                                    dx: Double(value.translation.width / scale),
                                    dy: Double(value.translation.height / scale)
                                )
                            }
                    )
                    .accessibilityLabel("Move group \(item.name)")
                    .help("Move structural group \(item.name)")
            }
        }
    }

    private func resizeHandleSpecs(for kind: String) -> [ResizeHandleSpec] {
        if kind == "text" {
            return [
                ResizeHandleSpec(id: "left", xFactor: 0, yFactor: 0.5, horizontal: true, vertical: false),
                ResizeHandleSpec(id: "right", xFactor: 1, yFactor: 0.5, horizontal: true, vertical: false),
            ]
        }
        guard ["rect", "image", "ellipse", "path"].contains(kind) else { return [] }
        return [
            ResizeHandleSpec(id: "top-left", xFactor: 0, yFactor: 0, horizontal: true, vertical: true),
            ResizeHandleSpec(id: "top", xFactor: 0.5, yFactor: 0, horizontal: false, vertical: true),
            ResizeHandleSpec(id: "top-right", xFactor: 1, yFactor: 0, horizontal: true, vertical: true),
            ResizeHandleSpec(id: "left", xFactor: 0, yFactor: 0.5, horizontal: true, vertical: false),
            ResizeHandleSpec(id: "right", xFactor: 1, yFactor: 0.5, horizontal: true, vertical: false),
            ResizeHandleSpec(id: "bottom-left", xFactor: 0, yFactor: 1, horizontal: true, vertical: true),
            ResizeHandleSpec(id: "bottom", xFactor: 0.5, yFactor: 1, horizontal: false, vertical: true),
            ResizeHandleSpec(id: "bottom-right", xFactor: 1, yFactor: 1, horizontal: true, vertical: true),
        ]
    }

    @ViewBuilder
    private var semanticResizeOverlay: some View {
        GeometryReader { proxy in
            if let scene = model.displayList,
               let item = model.selectedItem,
               let bounds = item.bounds,
               bounds.count == 4 {
                let innerWidth = max(proxy.size.width - 44, 1)
                let innerHeight = max(proxy.size.height - 44, 1)
                let scale = min(
                    innerWidth / CGFloat(scene.canvas.width),
                    innerHeight / CGFloat(scene.canvas.height)
                )
                let renderedWidth = CGFloat(scene.canvas.width) * scale
                let renderedHeight = CGFloat(scene.canvas.height) * scale
                let originX = 22 + (innerWidth - renderedWidth) / 2
                let originY = 22 + (innerHeight - renderedHeight) / 2

                ForEach(resizeHandleSpecs(for: item.primitive.kind)) { handle in
                    let baseX = originX + CGFloat(bounds[0] + bounds[2] * handle.xFactor) * scale
                    let baseY = originY + CGFloat(bounds[1] + bounds[3] * handle.yFactor) * scale
                    let isActive = activeResizeHandle == handle.id
                    let previewX = isActive && handle.horizontal ? resizePreview.width : 0
                    let previewY = isActive && handle.vertical ? resizePreview.height : 0
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.accentColor)
                        .overlay(RoundedRectangle(cornerRadius: 2).stroke(.black.opacity(0.8), lineWidth: 1))
                        .frame(
                            width: handle.horizontal && !handle.vertical ? 10 : 12,
                            height: handle.vertical && !handle.horizontal ? 10 : 12
                        )
                        .position(x: baseX + previewX, y: baseY + previewY)
                        .gesture(
                            DragGesture(minimumDistance: 1)
                                .onChanged { value in
                                    activeResizeHandle = handle.id
                                    resizePreview = CGSize(
                                        width: handle.horizontal ? value.translation.width : 0,
                                        height: handle.vertical ? value.translation.height : 0
                                    )
                                }
                                .onEnded { value in
                                    resizePreview = .zero
                                    activeResizeHandle = nil
                                    guard scale > 0 else { return }
                                    model.resize(
                                        node: item.id,
                                        handle: handle.id,
                                        dx: handle.horizontal ? Double(value.translation.width / scale) : 0,
                                        dy: handle.vertical ? Double(value.translation.height / scale) : 0
                                    )
                                }
                        )
                        .accessibilityLabel("Resize \(item.name) from \(handle.id) handle")
                        .help("Resize \(item.name) from \(handle.id)")
                }
            }
        }
    }

    private var sourceBench: some View {
        VStack(spacing: 0) {
            railHeader("SOURCE", trailing: model.isDirty ? "UNSAVED" : "SYNCED")
            TextEditor(text: $model.sourceText)
                .font(.system(size: 11.5, design: .monospaced))
                .scrollContentBackground(.hidden)
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(Color(nsColor: .textBackgroundColor).opacity(0.48))
                .onChange(of: model.sourceText) { _, _ in model.markDirty() }
        }
    }

    private var intentInspector: some View {
        VStack(alignment: .leading, spacing: 0) {
            railHeader("INTENT", trailing: model.selectedItem?.primitive.kind.uppercased() ?? "—")
            if let item = model.selectedItem, let record = model.displayList?.objectMap[item.id] {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(item.name).font(.system(size: 18, weight: .semibold))
                            Text(item.id).font(.system(size: 10, design: .monospaced)).foregroundStyle(.secondary)
                        }
                        inspectorRule
                        inspectorSection("ROLES") {
                            Text(item.roles.isEmpty ? "none" : item.roles.joined(separator: "  /  "))
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        inspectorSection("RESOLVED BOUNDS") {
                            Text(boundsString(item.bounds))
                                .font(.system(size: 11, design: .monospaced))
                        }
                        if !model.selectedConstraints.isEmpty {
                            inspectorSection("CONSTRAINTS") {
                                VStack(alignment: .leading, spacing: 8) {
                                    ForEach(model.selectedConstraints) { constraint in
                                        ConstraintEditorRow(
                                            constraint: constraint,
                                            isSelected: model.selectedConstraintID == constraint.id,
                                            onSelect: { model.selectedConstraintID = constraint.id },
                                            onRelation: { model.editConstraint(sourceOrder: constraint.sourceOrder, relation: $0) },
                                            onStrength: { model.editConstraint(sourceOrder: constraint.sourceOrder, strength: $0) },
                                            onOffset: { model.editConstraint(sourceOrder: constraint.sourceOrder, offset: $0) },
                                            onRemove: { model.removeConstraint(sourceOrder: constraint.sourceOrder) }
                                        )
                                        .id("\(constraint.id)-\(constraint.relation)-\(constraint.strength)-\(constraint.offset)")
                                    }
                                }
                            }
                        }
                        inspectorSection("ADD CONSTRAINT") {
                            newConstraintEditor(selectedNode: item.id)
                        }
                        if let plan = model.lastManipulationPlan, plan.nodeId == item.id {
                            inspectorSection("DIRECT MANIPULATION") {
                                if plan.allowed {
                                    if !plan.instanceParameterEdits.isEmpty {
                                        Text("component-parameter drag")
                                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                        ForEach(plan.instanceParameterEdits) { edit in
                                            Text("\(edit.instanceId).\(edit.parameter) → \(edit.targetProperty) · \(edit.axis.uppercased())")
                                                .font(.system(size: 9.5, design: .monospaced))
                                                .foregroundStyle(.secondary)
                                        }
                                    } else if plan.constraintEdits.isEmpty {
                                        Text("authored position edit")
                                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                        Text("Constraints preserved")
                                            .font(.system(size: 10, design: .monospaced))
                                            .foregroundStyle(.secondary)
                                    } else {
                                        Text("relation-preserving drag")
                                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                        ForEach(plan.constraintEdits) { edit in
                                            Text("\(edit.constraintId) · \(edit.axis.uppercased()) · \(edit.previousOffset) → \(edit.nextOffset)\(edit.unit ?? "")")
                                                .font(.system(size: 9.5, design: .monospaced))
                                                .foregroundStyle(.secondary)
                                        }
                                    }
                                } else if plan.blocked.isEmpty {
                                    Text(plan.note)
                                        .font(.system(size: 10, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                } else {
                                    ForEach(plan.blocked) { block in
                                        Text("\(block.axis.uppercased()) · \(block.constraintIds.joined(separator: ", "))")
                                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                        Text(block.reason)
                                            .font(.system(size: 9.5, design: .monospaced))
                                            .foregroundStyle(.secondary)
                                    }
                                }
                            }
                        }
                        if let plan = model.lastResizePlan, plan.nodeId == item.id {
                            inspectorSection("RESIZE") {
                                Text(plan.allowed ? "semantic size edit" : "resize blocked")
                                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                Text(plan.note)
                                    .font(.system(size: 9.5, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                ForEach(plan.constraintEdits) { edit in
                                    Text("\(edit.constraintId) · \(edit.previousOffset) → \(edit.nextOffset)\(edit.unit ?? "")")
                                        .font(.system(size: 9.5, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                }
                                ForEach(plan.instanceParameterEdits) { edit in
                                    Text("\(edit.instanceId).\(edit.parameter) → \(edit.targetProperty) · \(edit.axis.uppercased())")
                                        .font(.system(size: 9.5, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                }
                                ForEach(plan.blocked) { block in
                                    Text("\(block.axis.uppercased()) · \(block.constraintIds.joined(separator: ", "))")
                                        .font(.system(size: 9.5, weight: .semibold, design: .monospaced))
                                }
                            }
                        }
                        if let hierarchy = model.selectedHierarchyNode {
                            inspectorSection("HIERARCHY") {
                                VStack(alignment: .leading, spacing: 7) {
                                    Text(hierarchy.parent.map { "parent · \($0)" } ?? "parent · scene root")
                                        .font(.system(size: 10.5, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                    if !hierarchy.members.isEmpty {
                                        Text("members · \(hierarchy.members.joined(separator: ", "))")
                                            .font(.system(size: 9.5, design: .monospaced))
                                            .foregroundStyle(.secondary)
                                            .fixedSize(horizontal: false, vertical: true)
                                    }
                                    Menu("Move to…") {
                                        Button("Scene root") { model.reparentSelected(to: nil) }
                                            .disabled(hierarchy.parent == nil)
                                        if !model.availableHierarchyParents.isEmpty { Divider() }
                                        ForEach(model.availableHierarchyParents) { group in
                                            Button(group.name) { model.reparentSelected(to: group.id) }
                                                .disabled(hierarchy.parent == group.id)
                                        }
                                    }
                                    .disabled(model.isDirty)
                                    .help("Change structural parent without changing paint order or geometry")
                                }
                            }
                        }
                        inspectorSection("SOURCE") {
                            Text("line \(record.provenance.line) · \(record.provenance.authored ? "authored" : "generated")")
                                .font(.system(size: 11, design: .monospaced))
                            Text(URL(fileURLWithPath: record.provenance.source).lastPathComponent)
                                .font(.system(size: 10, design: .monospaced)).foregroundStyle(.secondary)
                        }
                        if let diff = model.lastVisualDiff,
                           let change = diff.changedObjects.first(where: { $0.id == item.id }) {
                            inspectorSection("LAST CHANGE") {
                                Text(change.changedFields.map { $0.uppercased() }.joined(separator: " · "))
                                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                Text("\(visualDiffRevision(diff))  ·  \(visualChangeSummary(change))")
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(.secondary)
                            }
                        }
                        inspectorRule
                        VStack(alignment: .leading, spacing: 8) {
                            Text("SURGICAL PATCH")
                                .font(.system(size: 9, weight: .bold, design: .monospaced))
                                .foregroundStyle(.secondary)
                            TextField("property  e.g. x, fill, ry", text: $patchProperty)
                                .textFieldStyle(.plain)
                                .padding(7)
                                .background(Color(nsColor: .textBackgroundColor).opacity(0.55))
                            TextField("value", text: $patchValue)
                                .textFieldStyle(.plain)
                                .padding(7)
                                .background(Color(nsColor: .textBackgroundColor).opacity(0.55))
                                .onSubmit {
                                    let property = patchProperty.trimmingCharacters(in: .whitespaces)
                                    guard !property.isEmpty, !patchValue.isEmpty else { return }
                                    model.setProperty(node: item.id, property: property, value: patchValue)
                                }
                            Button("Apply through Glyph source") {
                                model.setProperty(node: item.id, property: patchProperty, value: patchValue)
                            }
                            .buttonStyle(.borderedProminent)
                            .keyboardShortcut(.defaultAction)
                            .accessibilityIdentifier("glyph-apply-patch")
                            .disabled(patchProperty.trimmingCharacters(in: .whitespaces).isEmpty || patchValue.isEmpty)
                        }
                    }
                    .padding(14)
                }
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Select an object")
                        .font(.system(size: 15, weight: .semibold))
                    Text("The inspector follows stable Glyph IDs, not renderer layers.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(14)
                Spacer()
            }
        }
        .background(Color(nsColor: .underPageBackgroundColor).opacity(0.55))
    }

    private var componentExtractionSheet: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text("MAKE COMPONENT")
                        .font(.system(size: 12, weight: .bold, design: .monospaced))
                    Text("Moves the selected contiguous authored layers into a reusable local component without changing visual geometry.")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
            }
            Divider()
            TextField("component name", text: $extractedComponentName)
                .textFieldStyle(.roundedBorder)
            TextField("instance name", text: $extractedInstanceName)
                .textFieldStyle(.roundedBorder)
            Text("\(model.selectedIDs.count) selected layer\(model.selectedIDs.count == 1 ? "" : "s") · references or non-contiguous paint order fail closed")
                .font(.system(size: 9, design: .monospaced))
                .foregroundStyle(.secondary)
            HStack {
                Spacer()
                Button("Cancel") { showComponentExtraction = false }
                Button("Extract through Glyph source") {
                    let component = extractedComponentName.trimmingCharacters(in: .whitespaces)
                    let instance = extractedInstanceName.trimmingCharacters(in: .whitespaces)
                    model.extractSelectionAsComponent(component: component, instance: instance)
                    if model.error == nil {
                        showComponentExtraction = false
                        extractedComponentName = ""
                        extractedInstanceName = ""
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(
                    extractedComponentName.trimmingCharacters(in: .whitespaces).isEmpty
                        || extractedInstanceName.trimmingCharacters(in: .whitespaces).isEmpty
                        || !model.canExtractComponentSelection
                )
            }
        }
        .padding(16)
    }

    private var componentLibrarySelection: ComponentCatalogEntry? {
        model.components.first(where: { $0.name == componentSelection }) ?? model.components.first
    }

    private func componentValueBinding(_ key: String) -> Binding<String> {
        Binding(
            get: { componentBindingValues[key] ?? "" },
            set: { componentBindingValues[key] = $0 }
        )
    }

    private func selectedParameterBinding(_ key: String) -> Binding<String> {
        Binding(
            get: { selectedInstanceParameterValues[key] ?? "" },
            set: { selectedInstanceParameterValues[key] = $0 }
        )
    }

    private var componentAddReady: Bool {
        guard let component = componentLibrarySelection,
              !instanceName.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        let missingParameter = component.parameters.contains {
            $0.required && (componentBindingValues["param:\($0.name)"] ?? "").isEmpty
        }
        let missingSlot = component.slots.contains {
            $0.required && (componentBindingValues["slot:\($0.name)"] ?? "").isEmpty
        }
        return !missingParameter && !missingSlot && !model.isDirty
    }

    private var componentLibrarySheet: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text("COMPONENT LIBRARY")
                        .font(.system(size: 12, weight: .bold, design: .monospaced))
                    Text("Source-authoritative instances · \(model.components.count) available")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Done") { showComponentLibrary = false }
            }
            .padding(16)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    componentCreateEditor
                    if model.selectedInstanceOwnerID != nil {
                        Divider()
                        selectedInstanceEditor
                    }
                }
                .padding(16)
            }
        }
    }

    @ViewBuilder
    private var componentCreateEditor: some View {
        if let component = componentLibrarySelection {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("ADD INSTANCE")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                    Spacer()
                    Text("\(component.nodeCount) node\(component.nodeCount == 1 ? "" : "s")")
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                Picker("Component", selection: $componentSelection) {
                    ForEach(model.components) { entry in
                        Text(entry.name).tag(entry.name)
                    }
                }
                .onChange(of: componentSelection) { _, _ in
                    componentBindingValues.removeAll()
                    instanceVariant = ""
                }
                TextField("instance name", text: $instanceName)
                    .textFieldStyle(.roundedBorder)
                HStack {
                    TextField("x · optional", text: $instanceX).textFieldStyle(.roundedBorder)
                    TextField("y · optional", text: $instanceY).textFieldStyle(.roundedBorder)
                    TextField("scale · optional", text: $instanceScale).textFieldStyle(.roundedBorder)
                }
                if !component.parameters.isEmpty {
                    componentBindingEditor(title: "PARAMETERS", prefix: "param", bindings: component.parameters)
                }
                if !component.slots.isEmpty {
                    componentBindingEditor(title: "SLOTS", prefix: "slot", bindings: component.slots)
                }
                if !component.variants.isEmpty {
                    Picker("Variant", selection: $instanceVariant) {
                        Text("Default").tag("")
                        ForEach(component.variants, id: \.self) { Text($0).tag($0) }
                    }
                }
                Button("Add through Glyph source") {
                    var parameters: [String: String] = [:]
                    var slots: [String: String] = [:]
                    for binding in component.parameters {
                        let value = componentBindingValues["param:\(binding.name)"] ?? ""
                        if !value.isEmpty { parameters[binding.name] = value }
                    }
                    for binding in component.slots {
                        let value = componentBindingValues["slot:\(binding.name)"] ?? ""
                        if !value.isEmpty { slots[binding.name] = value }
                    }
                    model.addComponentInstance(
                        component: component.name,
                        name: instanceName.trimmingCharacters(in: .whitespaces),
                        x: instanceX, y: instanceY, scale: instanceScale,
                        parameters: parameters, slots: slots,
                        variant: instanceVariant.isEmpty ? nil : instanceVariant
                    )
                    instanceName = ""
                }
                .buttonStyle(.borderedProminent)
                .disabled(!componentAddReady)
            }
        } else {
            Text("No components are available in this project.")
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func componentBindingEditor(
        title: String,
        prefix: String,
        bindings: [ComponentBindingSummary]
    ) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title).font(.system(size: 9, weight: .bold, design: .monospaced))
            ForEach(bindings) { binding in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(binding.name + (binding.required ? " *" : ""))
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        Text("\(binding.valueType) → \(binding.target)")
                            .font(.system(size: 8.5, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    TextField(binding.required ? "required" : "use default", text: componentValueBinding("\(prefix):\(binding.name)"))
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 170)
                }
            }
        }
    }

    @ViewBuilder
    private var selectedInstanceEditor: some View {
        if let instance = model.selectedInstanceOwnerID,
           let componentName = model.selectedInstanceComponentName,
           let component = model.components.first(where: { $0.name == componentName }) {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("SELECTED INSTANCE")
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                        Text("\(instance) · \(componentName)")
                            .font(.system(size: 9, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("Remove", role: .destructive) {
                        model.removeSelectedInstance()
                    }
                    .disabled(!model.canRemoveSelectedInstance)
                }
                ForEach(component.parameters) { binding in
                    HStack {
                        Text("param \(binding.name)")
                            .font(.system(size: 10, design: .monospaced))
                        Spacer()
                        TextField("new \(binding.valueType) value", text: selectedParameterBinding("param:\(binding.name)"))
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 180)
                        Button("Set") {
                            let key = "param:\(binding.name)"
                            let value = selectedInstanceParameterValues[key] ?? ""
                            model.setInstanceParameter(instance: instance, parameter: binding.name, value: value)
                        }
                        .disabled((selectedInstanceParameterValues["param:\(binding.name)"] ?? "").isEmpty)
                    }
                }
                ForEach(component.slots) { binding in
                    HStack {
                        Text("slot \(binding.name)")
                            .font(.system(size: 10, design: .monospaced))
                        Spacer()
                        TextField("new \(binding.valueType) value", text: selectedParameterBinding("slot:\(binding.name)"))
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 180)
                        Button("Set") {
                            let key = "slot:\(binding.name)"
                            let value = selectedInstanceParameterValues[key] ?? ""
                            model.setInstanceSlot(instance: instance, slot: binding.name, value: value)
                        }
                        .disabled((selectedInstanceParameterValues["slot:\(binding.name)"] ?? "").isEmpty)
                    }
                }
            }
        }
    }

    private var statusBar: some View {
        HStack(spacing: 8) {
            Circle().fill(model.lastCheckOK ? Color.green.opacity(0.8) : Color.secondary.opacity(0.4)).frame(width: 6, height: 6)
            Text(model.status).font(.system(size: 10.5, design: .monospaced)).foregroundStyle(.secondary)
            Spacer()
            if let url = model.sourceURL {
                Text(url.path).font(.system(size: 9.5, design: .monospaced)).foregroundStyle(.tertiary).lineLimit(1)
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 28)
    }

    private func railHeader(_ title: String, trailing: String) -> some View {
        HStack {
            Text(title).font(.system(size: 9, weight: .bold, design: .monospaced)).tracking(0.8)
            Spacer()
            Text(trailing).font(.system(size: 8.5, design: .monospaced)).foregroundStyle(.tertiary).lineLimit(1)
        }
        .padding(.horizontal, 11)
        .frame(height: 30)
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.42))
    }

    private var inspectorRule: some View { Rectangle().fill(.quaternary).frame(height: 1) }

    private func inspectorSection<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.system(size: 9, weight: .bold, design: .monospaced)).foregroundStyle(.secondary)
            content()
        }
    }

    @ViewBuilder
    private func newConstraintEditor(selectedNode: String) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            TextField("lhs · \(selectedNode).center_x", text: $newConstraintLHS)
                .textFieldStyle(.roundedBorder)
            HStack(spacing: 6) {
                Picker("relation", selection: $newConstraintRelation) {
                    Text("==").tag("eq"); Text(">=").tag("ge"); Text("<=").tag("le")
                }
                .labelsHidden().frame(width: 64)
                TextField("rhs · canvas.center_x", text: $newConstraintRHS)
                    .textFieldStyle(.roundedBorder)
            }
            HStack(spacing: 6) {
                TextField("offset · optional", text: $newConstraintOffset)
                    .textFieldStyle(.roundedBorder)
                Picker("strength", selection: $newConstraintStrength) {
                    Text("required").tag("required"); Text("strong").tag("strong")
                    Text("medium").tag("medium"); Text("weak").tag("weak")
                }
                .labelsHidden().frame(width: 92)
            }
            Button("Add authored constraint") {
                model.addConstraint(
                    lhs: newConstraintLHS.trimmingCharacters(in: .whitespaces),
                    relation: newConstraintRelation,
                    rhs: newConstraintRHS.trimmingCharacters(in: .whitespaces),
                    offset: newConstraintOffset.trimmingCharacters(in: .whitespaces).isEmpty ? nil : newConstraintOffset.trimmingCharacters(in: .whitespaces),
                    strength: newConstraintStrength
                )
            }
            .buttonStyle(.bordered)
            .disabled(newConstraintLHS.trimmingCharacters(in: .whitespaces).isEmpty || newConstraintRHS.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    private func visualDiffRevision(_ diff: VisualDiffReport) -> String {
        diff.beforeRevision == diff.afterRevision
            ? "source edit"
            : "r\(diff.beforeRevision) → r\(diff.afterRevision)"
    }

    private func visualChangeSummary(_ change: VisualObjectDiff) -> String {
        var parts: [String] = []
        if change.moved { parts.append("moved") }
        if change.resized { parts.append("resized") }
        if change.styleChanged { parts.append("restyled") }
        if change.contentChanged { parts.append("content") }
        if change.semanticChanged { parts.append("semantic") }
        if change.geometryChanged && !change.moved && !change.resized { parts.append("geometry") }
        return parts.isEmpty ? change.status : parts.joined(separator: ", ")
    }

    private func boundsString(_ bounds: [Double]?) -> String {
        guard let b = bounds, b.count == 4 else { return "not available" }
        return String(format: "x %.1f   y %.1f\nw %.1f   h %.1f", b[0], b[1], b[2], b[3])
    }
}

private struct ConstraintEditorRow: View {
    let constraint: ConstraintRecord
    let isSelected: Bool
    let onSelect: () -> Void
    let onRelation: (String) -> Void
    let onStrength: (String) -> Void
    let onOffset: (String) -> Void
    let onRemove: () -> Void
    @State private var offsetText: String

    init(
        constraint: ConstraintRecord,
        isSelected: Bool,
        onSelect: @escaping () -> Void,
        onRelation: @escaping (String) -> Void,
        onStrength: @escaping (String) -> Void,
        onOffset: @escaping (String) -> Void,
        onRemove: @escaping () -> Void
    ) {
        self.constraint = constraint
        self.isSelected = isSelected
        self.onSelect = onSelect
        self.onRelation = onRelation
        self.onStrength = onStrength
        self.onOffset = onOffset
        self.onRemove = onRemove
        _offsetText = State(initialValue: Self.sourceOffset(constraint))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button(action: onSelect) {
                HStack(spacing: 6) {
                    Text("C\(constraint.sourceOrder)")
                        .font(.system(size: 9, weight: .bold, design: .monospaced))
                    Text(constraint.displayExpression)
                        .font(.system(size: 9.5, design: .monospaced))
                        .lineLimit(2)
                    Spacer(minLength: 0)
                }
                .padding(6)
                .background(isSelected ? Color.accentColor.opacity(0.12) : Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)

            HStack(spacing: 5) {
                Menu(constraint.relationSymbol) {
                    Button("==") { onRelation("eq") }
                    Button(">=") { onRelation("ge") }
                    Button("<=") { onRelation("le") }
                }
                Menu(constraint.strength) {
                    Button("required") { onStrength("required") }
                    Button("strong") { onStrength("strong") }
                    Button("medium") { onStrength("medium") }
                    Button("weak") { onStrength("weak") }
                }
                TextField("offset", text: $offsetText)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 58)
                    .onSubmit { onOffset(offsetText.isEmpty ? "0" : offsetText) }
                Button("Set") { onOffset(offsetText.isEmpty ? "0" : offsetText) }
                    .buttonStyle(.borderless)
                Button(role: .destructive, action: onRemove) {
                    Image(systemName: "trash")
                }
                .buttonStyle(.borderless)
                .accessibilityLabel("Remove constraint \(constraint.sourceOrder)")
            }
            .font(.system(size: 9.5, design: .monospaced))
        }
    }

    private static func sourceOffset(_ constraint: ConstraintRecord) -> String {
        guard abs(constraint.offset) > Double.ulpOfOne else { return "" }
        let value = constraint.offset.rounded() == constraint.offset
            ? String(Int(constraint.offset))
            : String(format: "%g", constraint.offset)
        return "\(value)\(constraint.offsetUnit ?? "")"
    }
}
