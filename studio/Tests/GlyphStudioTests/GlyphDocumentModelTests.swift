import Foundation
import XCTest
@testable import GlyphStudio

final class GlyphDocumentModelTests: XCTestCase {
    @MainActor
    func testDragUndoRedoRelationOffsetAndConstraintGuardResolution() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("drag.glyph")
        let source = #"""
        scene "Studio Drag" {
          canvas 320 220;
          rect free @id("free") { x 20px; y 30px; width 80px; height 60px; fill "#336699"; }
          rect related @id("related") { x 180px; y 120px; width 70px; height 50px; fill "#995544"; }
          rect ambiguous @id("ambiguous") { x 40px; y 140px; width 60px; height 40px; fill "#557755"; }
          constrain related.center_x == canvas.center_x @required;
          constrain ambiguous.center_x == canvas.center_x @required;
          constrain ambiguous.left >= 0 @strong;
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)

        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertFalse(model.canUndo)

        model.drag(node: "free", dx: 12, dy: -7)
        let moved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(moved.contains("x 32px;"), moved)
        XCTAssertTrue(moved.contains("y 23px;"), moved)
        XCTAssertTrue(model.canUndo)
        XCTAssertTrue(model.lastManipulationPlan?.allowed == true)
        model.undoSemanticEdit()
        let undone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(undone.contains("x 20px;"), undone)
        XCTAssertTrue(undone.contains("y 30px;"), undone)
        XCTAssertTrue(model.canRedo)

        model.redoSemanticEdit()
        let redone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(redone.contains("x 32px;"), redone)
        XCTAssertTrue(redone.contains("y 23px;"), redone)

        model.resize(node: "free", dw: 15, dh: -5)
        let resized = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resized.contains("width 95px;"), resized)
        XCTAssertTrue(resized.contains("height 55px;"), resized)
        XCTAssertEqual(model.lastResizePlan?.allowed, true)
        model.undoSemanticEdit()
        let resizeUndone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resizeUndone.contains("width 80px;"), resizeUndone)
        XCTAssertTrue(resizeUndone.contains("height 60px;"), resizeUndone)
        model.redoSemanticEdit()
        let resizeRedone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resizeRedone.contains("width 95px;"), resizeRedone)
        XCTAssertTrue(resizeRedone.contains("height 55px;"), resizeRedone)

        model.drag(node: "related", dx: 12, dy: 0)
        let relationMoved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(
            relationMoved.contains("constrain related.center_x == canvas.center_x + 12 @required;"),
            relationMoved
        )
        XCTAssertEqual(model.lastManipulationPlan?.allowed, true)
        XCTAssertEqual(model.lastManipulationPlan?.constraintEdits.first?.constraintId, "constraint.1")
        model.undoSemanticEdit()
        let relationUndone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertFalse(relationUndone.contains("canvas.center_x + 12 @required;"), relationUndone)
        model.redoSemanticEdit()
        let relationRedone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(relationRedone.contains("canvas.center_x + 12 @required;"), relationRedone)

        model.drag(node: "ambiguous", dx: 12, dy: 0)
        let guardedMoved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(
            guardedMoved.contains("constrain ambiguous.center_x == canvas.center_x + 12 @required;"),
            guardedMoved
        )
        XCTAssertTrue(guardedMoved.contains("constrain ambiguous.left >= 0 @strong;"), guardedMoved)
        XCTAssertEqual(model.lastManipulationPlan?.allowed, true)
        XCTAssertEqual(model.lastManipulationPlan?.constraintEdits.first?.constraintId, "constraint.2")

        model.undoSemanticEdit()
        let beforeBlockedDrag = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertEqual(beforeBlockedDrag, relationRedone)
        model.drag(node: "ambiguous", dx: -200, dy: 0)
        let afterBlockedDrag = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertEqual(afterBlockedDrag, beforeBlockedDrag)
        XCTAssertEqual(model.lastManipulationPlan?.allowed, false)
        XCTAssertEqual(
            model.lastManipulationPlan?.blocked.first?.constraintIds,
            ["constraint.2", "constraint.3"]
        )
        XCTAssertTrue(model.status.contains("constraint.2"), model.status)
        XCTAssertTrue(model.status.contains("constraint.3"), model.status)
    }
}

extension GlyphDocumentModelTests {
    @MainActor
    func testComponentParameterDragAndResizeRoundTrip() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-component-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("component.glyph")
        let source = #"""
        scene "Studio Component" {
          canvas 500 300;
          component card {
            param px: Length -> bg.x = 10px;
            param w: Length -> bg.width = 100px;
            rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; }
          }
          instance card card1 @id("card1") {
            x 100px; scale 2;
            param px 15px; param w 120px;
          }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)

        model.drag(node: "card1/bg", dx: 20, dy: 0)
        let moved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(moved.contains("param px 25px;"), moved)
        XCTAssertEqual(model.lastManipulationPlan?.allowed, true)
        XCTAssertEqual(model.lastManipulationPlan?.instanceParameterEdits.first?.parameter, "px")
        XCTAssertEqual(model.lastManipulationPlan?.instanceParameterEdits.first?.instanceId, "card1")

        model.resize(node: "card1/bg", dw: 20, dh: 0)
        let resized = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resized.contains("param w 130px;"), resized)
        XCTAssertEqual(model.lastResizePlan?.allowed, true)
        XCTAssertEqual(model.lastResizePlan?.instanceParameterEdits.first?.parameter, "w")

        model.undoSemanticEdit()
        let resizeUndone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resizeUndone.contains("param w 120px;"), resizeUndone)
        XCTAssertTrue(resizeUndone.contains("param px 25px;"), resizeUndone)
    }
}

extension GlyphDocumentModelTests {
    @MainActor
    func testPathDragAndResizeUseAffineSourceTransformsAndUndo() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-path-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("path.glyph")
        let source = #"""
        scene "Studio Path" {
          canvas 300 200;
          path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; fill "#112233"; }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.displayList?.objectMap["mark"]?.bounds, [10, 20, 50, 50])
        model.drag(node: "mark", dx: 15, dy: -5)
        let moved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(moved.contains("translate_x 15;"), moved)
        XCTAssertTrue(moved.contains("translate_y -5;"), moved)
        XCTAssertTrue(moved.contains("d \"M10 20 L60 20 L60 70 Z\";"), moved)
        XCTAssertEqual(model.lastManipulationPlan?.allowed, true)
        XCTAssertEqual(model.displayList?.objectMap["mark"]?.bounds, [25, 15, 50, 50])

        model.resize(node: "mark", dw: 25, dh: 10)
        let resized = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resized.contains("scale_x 1.5;"), resized)
        XCTAssertTrue(resized.contains("scale_y 1.2;"), resized)
        XCTAssertTrue(resized.contains("d \"M10 20 L60 20 L60 70 Z\";"), resized)
        XCTAssertEqual(model.lastResizePlan?.allowed, true)
        XCTAssertEqual(model.displayList?.objectMap["mark"]?.bounds, [25, 15, 75, 60])

        model.undoSemanticEdit()
        let resizeUndone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resizeUndone.contains("translate_x 15;"), resizeUndone)
        XCTAssertFalse(resizeUndone.contains("scale_x 1.5;"), resizeUndone)
        XCTAssertEqual(model.displayList?.objectMap["mark"]?.bounds, [25, 15, 50, 50])

        model.undoSemanticEdit()
        let dragUndone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertFalse(dragUndone.contains("translate_x 15;"), dragUndone)
        XCTAssertEqual(model.displayList?.objectMap["mark"]?.bounds, [10, 20, 50, 50])
    }
}

extension GlyphDocumentModelTests {
    @MainActor
    func testConstraintEditingResolvesAmbiguousDragThroughAuthoredSource() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-constraint-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("constraints.glyph")
        let source = #"""
        scene "Studio Constraints" {
          canvas 320 220;
          rect ambiguous @id("ambiguous") { x 40px; y 140px; width 60px; height 40px; }
          constrain ambiguous.center_x == canvas.center_x @required;
          constrain ambiguous.left >= 0 @strong;
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        model.selectedID = "ambiguous"
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.constraints.count, 2)
        XCTAssertEqual(model.selectedConstraints.map(\.id), ["constraint.1", "constraint.2"])

        model.editConstraint(sourceOrder: 1, strength: "strong", offset: "5px")
        let edited = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(edited.contains("canvas.center_x + 5px @strong;"), edited)
        XCTAssertEqual(model.constraints[0].strength, "strong")

        model.undoSemanticEdit()
        let restored = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(restored.contains("canvas.center_x @required;"), restored)
        XCTAssertEqual(model.constraints[0].strength, "required")

        model.removeConstraint(sourceOrder: 2)
        XCTAssertEqual(model.constraints.count, 1)
        let removed = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertFalse(removed.contains("ambiguous.left >= 0"), removed)

        model.addConstraint(
            lhs: "ambiguous.top",
            relation: "ge",
            rhs: "0",
            strength: "medium"
        )
        XCTAssertEqual(model.constraints.count, 2)
        XCTAssertTrue(model.sourceText.contains("constrain ambiguous.top >= 0 @medium;"))

        model.drag(node: "ambiguous", dx: 12, dy: 0)
        XCTAssertEqual(model.lastManipulationPlan?.allowed, true)
        let moved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(moved.contains("canvas.center_x + 12 @required;"), moved)

        model.undoSemanticEdit()
        XCTAssertFalse(model.sourceText.contains("canvas.center_x + 12 @required;"))
        XCTAssertEqual(model.constraints.count, 2)
    }
}

extension GlyphDocumentModelTests {
    @MainActor
    func testTextFrameResizeEditsWrapWidthAndFailsClosedVertically() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-text-resize-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("text-resize.glyph")
        let source = #"""
        scene "Studio Text Resize" {
          canvas 420 240;
          text copy @id("copy") {
            content "Semantic text frames reflow without scaling type";
            x 30px; y 70px; size 22px; max_width 160px; line_height 28px; fill "#111111";
          }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.displayList?.objectMap["copy"]?.bounds?[2], 160)

        model.resize(node: "copy", dw: 50, dh: 0)
        let resized = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resized.contains("max_width 210px;"), resized)
        XCTAssertTrue(resized.contains("size 22px;"), resized)
        XCTAssertEqual(model.lastResizePlan?.allowed, true)
        XCTAssertEqual(model.displayList?.objectMap["copy"]?.bounds?[2], 210)

        model.undoSemanticEdit()
        let undone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(undone.contains("max_width 160px;"), undone)
        XCTAssertEqual(model.displayList?.objectMap["copy"]?.bounds?[2], 160)

        let beforeBlocked = try String(contentsOf: sourceURL, encoding: .utf8)
        model.resize(node: "copy", dw: 0, dh: 20)
        let afterBlocked = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertEqual(afterBlocked, beforeBlocked)
        XCTAssertEqual(model.lastResizePlan?.allowed, false)
        XCTAssertTrue(
            model.lastResizePlan?.blocked.first?.reason.contains("vertical text resize") == true,
            model.lastResizePlan?.blocked.first?.reason ?? "missing block reason"
        )
    }
}

extension GlyphDocumentModelTests {
    @MainActor
    func testMultiEdgeHandleResizePreservesOppositeEdgeAndUndo() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-handle-resize-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("handle-resize.glyph")
        let source = #"""
        scene "Studio Handle Resize" {
          canvas 360 240;
          rect box @id("box") { x 20px; y 30px; width 100px; height 80px; fill "#334455"; }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.displayList?.objectMap["box"]?.bounds, [20, 30, 100, 80])

        model.resize(node: "box", handle: "left", dx: 15, dy: 0)
        let resized = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(resized.contains("x 35px;"), resized)
        XCTAssertTrue(resized.contains("width 85px;"), resized)
        XCTAssertEqual(model.lastHandleResizePlan?.allowed, true)
        XCTAssertEqual(model.lastHandleResizePlan?.handle, "left")
        XCTAssertEqual(model.displayList?.objectMap["box"]?.bounds, [35, 30, 85, 80])

        model.undoSemanticEdit()
        let undone = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(undone.contains("x 20px;"), undone)
        XCTAssertTrue(undone.contains("width 100px;"), undone)
        XCTAssertEqual(model.displayList?.objectMap["box"]?.bounds, [20, 30, 100, 80])
    }
}

extension GlyphDocumentModelTests {
    @MainActor
    func testStructuralGroupDragMovesMembersAndPreservesMembership() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-group-drag-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("group-drag.glyph")
        let source = #"""
        scene "Studio Group Drag" {
          canvas 420 260;
          rect left @id("left") { x 40px; y 50px; width 80px; height 40px; }
          rect right @id("right") { x 40px; y 120px; width 60px; height 30px; }
          constrain left.x == right.x @required;
          group pair @id("pair") { member left; member right; }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.displayList?.objectMap["pair"]?.bounds, [40, 50, 80, 100])

        model.drag(node: "pair", dx: 20, dy: -10)
        let moved = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(moved.contains("member left;"), moved)
        XCTAssertTrue(moved.contains("member right;"), moved)
        XCTAssertTrue(moved.contains("x 60px;"), moved)
        XCTAssertTrue(moved.contains("constrain left.x == right.x @required;"), moved)
        XCTAssertEqual(model.lastManipulationPlan?.allowed, true)
        XCTAssertEqual(model.lastManipulationPlan?.memberPlans?.count, 2)
        XCTAssertEqual(model.displayList?.objectMap["pair"]?.bounds, [60, 40, 80, 100])

        model.undoSemanticEdit()
        XCTAssertEqual(model.displayList?.objectMap["pair"]?.bounds, [40, 50, 80, 100])
    }
}


extension GlyphDocumentModelTests {
    @MainActor
    func testStudioMultiSelectGroupingAndUngroupRoundTrip() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-grouping-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("grouping.glyph")
        let source = #"""
        scene "Studio Grouping" {
          canvas 420 260;
          rect first @id("first") { x 40px; y 50px; width 80px; height 40px; }
          rect second @id("second") { x 150px; y 80px; width 60px; height 50px; }
          rect third @id("third") { x 250px; y 120px; width 40px; height 30px; }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)

        model.select("first")
        model.select("second", extending: true)
        XCTAssertEqual(model.selectedIDs, Set(["first", "second"]))
        XCTAssertTrue(model.canGroupSelection)

        model.groupSelection()
        let grouped = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertTrue(grouped.contains("group group {"), grouped)
        XCTAssertTrue(grouped.contains("member first;"), grouped)
        XCTAssertTrue(grouped.contains("member second;"), grouped)
        XCTAssertEqual(model.selectedID, "group")
        XCTAssertEqual(model.selectedIDs, Set(["group"]))
        XCTAssertTrue(model.canUngroupSelection)
        XCTAssertEqual(model.displayList?.objectMap["group"]?.bounds, [40, 50, 170, 80])

        model.ungroupSelected()
        let ungrouped = try String(contentsOf: sourceURL, encoding: .utf8)
        XCTAssertFalse(ungrouped.contains("group group {"), ungrouped)
        XCTAssertNil(model.displayList?.objectMap["group"])
        XCTAssertTrue(model.selectedIDs.isEmpty)

        model.undoSemanticEdit()
        XCTAssertEqual(model.displayList?.objectMap["group"]?.bounds, [40, 50, 170, 80])
    }
}


extension GlyphDocumentModelTests {
    @MainActor
    func testStudioSourceHierarchyReorderAndUndoRoundTrip() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-reorder-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("reorder.glyph")
        let source = #"""
        scene "Studio Reorder" {
          canvas 360 240;
          rect first @id("first") { x 20px; y 30px; width 100px; height 80px; fill "#334455"; }
          rect second @id("second") { x 40px; y 50px; width 100px; height 80px; fill "#556677"; }
          rect third @id("third") { x 60px; y 70px; width 100px; height 80px; fill "#778899"; }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["first", "second", "third"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["first", "second", "third"])

        model.select("second")
        XCTAssertTrue(model.canSendBackward)
        XCTAssertTrue(model.canBringForward)
        model.sendSelectedBackward()
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["second", "first", "third"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["second", "first", "third"])
        XCTAssertTrue(model.canUndo)

        model.undoSemanticEdit()
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["first", "second", "third"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["first", "second", "third"])

        model.bringSelectedForward()
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["first", "third", "second"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["first", "third", "second"])
    }
}


extension GlyphDocumentModelTests {
    @MainActor
    func testStudioReordersSelectedComponentChildByMovingItsInstanceOwner() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("glyph-studio-instance-reorder-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let sourceURL = directory.appendingPathComponent("instance-reorder.glyph")
        let source = #"""
        scene "Studio Instance Reorder" {
          canvas 420 260;
          component card {
            rect bg @id("bg") { x 0px; y 0px; width 80px; height 40px; fill "#334455"; }
            text label @id("label") { content "Card"; x 8px; y 24px; size 14px; fill "#ffffff"; }
          }
          rect first @id("first") { x 20px; y 30px; width 80px; height 60px; fill "#111111"; }
          instance card card1 @id("card1") { x 120px; y 30px; }
          rect second @id("second") { x 240px; y 30px; width 80px; height 60px; fill "#222222"; }
        }
        """#
        try source.write(to: sourceURL, atomically: true, encoding: .utf8)
        let model = GlyphDocumentModel()
        model.open(sourceURL)
        XCTAssertTrue(model.lastCheckOK)
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["first", "card1", "second"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["first", "card1/bg", "card1/label", "second"])

        model.select("card1/bg")
        XCTAssertTrue(model.canSendBackward)
        XCTAssertTrue(model.canBringForward)
        model.sendSelectedBackward()
        XCTAssertEqual(model.selectedID, "card1/bg")
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["card1", "first", "second"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["card1/bg", "card1/label", "first", "second"])

        model.undoSemanticEdit()
        XCTAssertEqual(model.sourceHierarchy.map(\.id), ["first", "card1", "second"])
        XCTAssertEqual(model.displayList?.items.map(\.id), ["first", "card1/bg", "card1/label", "second"])
    }
}
