import SwiftUI
import WebKit

struct SVGPreview: NSViewRepresentable {
    let svg: String
    let selectedID: String?
    let onSelect: (String) -> Void
    let onDrag: (String, Double, Double) -> Void

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.userContentController.add(context.coordinator, name: "glyphSelection")
        configuration.userContentController.add(context.coordinator, name: "glyphDrag")
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.setValue(false, forKey: "drawsBackground")
        view.navigationDelegate = context.coordinator
        context.coordinator.webView = view
        return view
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        if context.coordinator.loadedSVG != svg {
            context.coordinator.loadedSVG = svg
            let html = """
            <!doctype html><html><head><meta charset="utf-8"><style>
            html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}
            body{display:flex;align-items:center;justify-content:center}
            svg{width:100%;height:100%;max-width:100%;max-height:100%;display:block}
            .glyph-selected{filter:drop-shadow(0 0 7px rgba(255,202,87,.95)) drop-shadow(0 0 1px rgba(255,255,255,1));}
            [id][data-glyph-role]{cursor:grab;touch-action:none}
            .glyph-dragging{cursor:grabbing!important}
            </style></head><body>\(svg)<script>
            const root = document.querySelector('svg');
            const point = event => {
              const p = new DOMPoint(event.clientX, event.clientY);
              const matrix = root && root.getScreenCTM();
              return matrix ? p.matrixTransform(matrix.inverse()) : p;
            };
            let drag = null;
            document.addEventListener('pointerdown', event => {
              const target = event.target.closest('[id][data-glyph-role]');
              if (!target || !target.id) return;
              const start = point(event);
              drag = { id: target.id, start, target, pointerId: event.pointerId };
              target.setPointerCapture?.(event.pointerId);
              target.classList.add('glyph-dragging');
              window.webkit.messageHandlers.glyphSelection.postMessage(target.id);
              event.preventDefault();
            });
            document.addEventListener('pointerup', event => {
              if (!drag || drag.pointerId !== event.pointerId) return;
              const end = point(event);
              const dx = end.x - drag.start.x, dy = end.y - drag.start.y;
              drag.target.classList.remove('glyph-dragging');
              if (Math.hypot(dx, dy) >= 0.5) {
                window.webkit.messageHandlers.glyphDrag.postMessage({id: drag.id, dx, dy});
              }
              drag = null;
              event.preventDefault();
            });
            document.addEventListener('pointercancel', () => {
              if (drag) drag.target.classList.remove('glyph-dragging');
              drag = null;
            });
            </script></body></html>
            """
            webView.loadHTMLString(html, baseURL: nil)
        }
        context.coordinator.onSelect = onSelect
        context.coordinator.onDrag = onDrag
        context.coordinator.selectedID = selectedID
        context.coordinator.applySelection()
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "glyphSelection")
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "glyphDrag")
    }

    func makeCoordinator() -> Coordinator { Coordinator(onSelect: onSelect, onDrag: onDrag) }

    final class Coordinator: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
        weak var webView: WKWebView?
        var loadedSVG = ""
        var selectedID: String?
        var onSelect: (String) -> Void
        var onDrag: (String, Double, Double) -> Void

        init(
            onSelect: @escaping (String) -> Void,
            onDrag: @escaping (String, Double, Double) -> Void
        ) {
            self.onSelect = onSelect
            self.onDrag = onDrag
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            applySelection()
        }

        func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
            if message.name == "glyphSelection", let id = message.body as? String {
                onSelect(id)
                return
            }
            guard message.name == "glyphDrag",
                  let payload = message.body as? [String: Any],
                  let id = payload["id"] as? String,
                  let dx = payload["dx"] as? Double,
                  let dy = payload["dy"] as? Double else { return }
            onDrag(id, dx, dy)
        }

        func applySelection() {
            guard let webView else { return }
            let quoted = selectedID.map { "'\($0.replacingOccurrences(of: "'", with: "\\'"))'" } ?? "null"
            let script = """
            document.querySelectorAll('.glyph-selected').forEach(e=>e.classList.remove('glyph-selected'));
            const id=\(quoted); if(id){ const e=document.getElementById(id); if(e){e.classList.add('glyph-selected');} }
            """
            webView.evaluateJavaScript(script, completionHandler: nil)
        }
    }
}
