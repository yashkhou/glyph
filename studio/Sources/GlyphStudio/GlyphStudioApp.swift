import AppKit
import SwiftUI

final class GlyphStudioAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@main
struct GlyphStudioApp: App {
    @NSApplicationDelegateAdaptor(GlyphStudioAppDelegate.self) private var appDelegate
    @StateObject private var model = GlyphDocumentModel()

    var body: some Scene {
        WindowGroup("Glyph Studio") {
            ContentView(model: model)
                .preferredColorScheme(.dark)
                .task {
                    if model.sourceURL == nil {
                        let args = CommandLine.arguments.dropFirst()
                        if let first = args.first, first.hasSuffix(".glyph") {
                            model.open(URL(fileURLWithPath: first))
                        } else {
                            let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                            let candidates = [
                                cwd.appendingPathComponent("../examples/sakura_lake.glyph").standardizedFileURL,
                                cwd.appendingPathComponent("examples/sakura_lake.glyph").standardizedFileURL,
                                URL(fileURLWithPath: "/Users/joshuakhoury/Projects/Glyph/examples/sakura_lake.glyph"),
                            ]
                            if let sample = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) }) {
                                model.open(sample)
                            }
                        }
                    }
                }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1320, height: 850)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Open Glyph Source…") { model.openPanel() }.keyboardShortcut("o")
                Button("Import Image as Glyph…") { model.importImagePanel() }
                    .keyboardShortcut("i", modifiers: [.command, .shift])
                Button("Save Glyph Source") { model.saveSource() }.keyboardShortcut("s")
            }
            CommandGroup(replacing: .undoRedo) {
                Button("Undo Glyph Edit") { model.undoSemanticEdit() }
                    .keyboardShortcut("z", modifiers: [.command])
                    .disabled(!model.canUndo || model.isDirty)
                Button("Redo Glyph Edit") { model.redoSemanticEdit() }
                    .keyboardShortcut("z", modifiers: [.command, .shift])
                    .disabled(!model.canRedo || model.isDirty)
            }
        }
    }
}
