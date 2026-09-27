import AppKit
import CoreGraphics
import CoreText
import Foundation
import ImageIO
import Vision

struct VisionBox: Codable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

struct VisionObservation: Codable {
    let kind: String
    let text: String?
    let bbox: VisionBox
    let confidence: Double
    let color: String?
    let corner_radius: Double?
    let font_size: Double?
    let font_weight: String?
    let baseline_y: Double?
}

struct VisionManifest: Codable {
    let schema_version: Int
    let source: String
    let width: Int
    let height: Int
    let background: String
    let observations: [VisionObservation]
}

struct RGB {
    let r: Int
    let g: Int
    let b: Int

    var hex: String { String(format: "#%02x%02x%02x", r, g, b) }
    func distance(to other: RGB) -> Double {
        let dr = Double(r - other.r), dg = Double(g - other.g), db = Double(b - other.b)
        return sqrt(dr * dr + dg * dg + db * db)
    }
}
struct PixelSampler {
    let width: Int
    let height: Int
    let pixels: [UInt8]

    init?(image: CGImage) {
        let imageWidth = image.width
        let imageHeight = image.height
        let bytesPerRow = imageWidth * 4
        var buffer = [UInt8](repeating: 0, count: bytesPerRow * imageHeight)
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue
            | CGBitmapInfo.byteOrder32Big.rawValue
        let rendered = buffer.withUnsafeMutableBytes { raw -> Bool in
            guard let context = CGContext(
                data: raw.baseAddress,
                width: imageWidth,
                height: imageHeight,
                bitsPerComponent: 8,
                bytesPerRow: bytesPerRow,
                space: colorSpace,
                bitmapInfo: bitmapInfo
            ) else { return false }
            // Raw bitmap rows already align with the top-down coordinates used by
            // Glyph's Vision boxes. Flipping here mirrors all color samples vertically.
            context.draw(image, in: CGRect(x: 0, y: 0, width: imageWidth, height: imageHeight))
            return true
        }
        guard rendered else { return nil }
        width = imageWidth
        height = imageHeight
        pixels = buffer
    }

    func color(x: Int, y: Int) -> RGB {
        let px = min(max(x, 0), width - 1)
        let py = min(max(y, 0), height - 1)
        let index = (py * width + px) * 4
        return RGB(r: Int(pixels[index]), g: Int(pixels[index + 1]), b: Int(pixels[index + 2]))
    }
}

struct CompactRegionCandidate {
    let bbox: VisionBox
    let color: RGB
    let occupancy: Double
}

func quantizedColorKey(_ color: RGB, quantum: Int = 16) -> Int {
    ((color.r / quantum) << 16) | ((color.g / quantum) << 8) | (color.b / quantum)
}

func compactSolidRegions(_ sampler: PixelSampler, background: RGB) -> [CompactRegionCandidate] {
    let width = sampler.width
    let height = sampler.height
    let total = width * height
    let compactLimit = max(14, Int(Double(min(width, height)) * 0.18))
    let minimumDimension = max(6, Int(Double(min(width, height)) * 0.015))
    var visited = [Bool](repeating: false, count: total)
    var candidates: [CompactRegionCandidate] = []

    for start in 0..<total where !visited[start] {
        let startX = start % width
        let startY = start / width
        let seed = sampler.color(x: startX, y: startY)
        let key = quantizedColorKey(seed)
        var queue = [start]
        var head = 0
        visited[start] = true
        var minX = startX, maxX = startX, minY = startY, maxY = startY
        var count = 0
        var sumR: Int64 = 0, sumG: Int64 = 0, sumB: Int64 = 0

        while head < queue.count {
            let index = queue[head]
            head += 1
            let x = index % width
            let y = index / width
            let color = sampler.color(x: x, y: y)
            count += 1
            sumR += Int64(color.r); sumG += Int64(color.g); sumB += Int64(color.b)
            minX = min(minX, x); maxX = max(maxX, x)
            minY = min(minY, y); maxY = max(maxY, y)

            for dy in -1...1 {
                for dx in -1...1 where dx != 0 || dy != 0 {
                    let nx = x + dx, ny = y + dy
                    guard nx >= 0, ny >= 0, nx < width, ny < height else { continue }
                    let neighbor = ny * width + nx
                    guard !visited[neighbor] else { continue }
                    if quantizedColorKey(sampler.color(x: nx, y: ny)) == key {
                        visited[neighbor] = true
                        queue.append(neighbor)
                    }
                }
            }
        }

        let boxWidth = maxX - minX + 1
        let boxHeight = maxY - minY + 1
        guard boxWidth >= minimumDimension, boxHeight >= minimumDimension,
              boxWidth <= compactLimit, boxHeight <= compactLimit else { continue }
        let aspect = Double(boxWidth) / Double(boxHeight)
        guard (0.65...1.45).contains(aspect) else { continue }
        let occupancy = Double(count) / Double(boxWidth * boxHeight)
        guard occupancy >= 0.48 else { continue }
        let color = RGB(
            r: Int(sumR / Int64(count)),
            g: Int(sumG / Int64(count)),
            b: Int(sumB / Int64(count))
        )
        guard color.distance(to: background) >= 18.0 else { continue }
        candidates.append(CompactRegionCandidate(
            bbox: VisionBox(x: Double(minX), y: Double(minY), width: Double(boxWidth), height: Double(boxHeight)),
            color: color,
            occupancy: occupancy
        ))
    }
    return candidates
}

func medianColor(_ colors: [RGB]) -> RGB {
    guard !colors.isEmpty else { return RGB(r: 255, g: 255, b: 255) }
    let middle = colors.count / 2
    let rs = colors.map(\.r).sorted()
    let gs = colors.map(\.g).sorted()
    let bs = colors.map(\.b).sorted()
    return RGB(r: rs[middle], g: gs[middle], b: bs[middle])
}

func backgroundColor(_ sampler: PixelSampler) -> RGB {
    let inset = max(2, min(sampler.width, sampler.height) / 100)
    var colors: [RGB] = []
    for (x, y) in [
        (inset, inset),
        (sampler.width - 1 - inset, inset),
        (inset, sampler.height - 1 - inset),
        (sampler.width - 1 - inset, sampler.height - 1 - inset),
        (sampler.width / 2, inset),
        (sampler.width / 2, sampler.height - 1 - inset),
    ] {
        colors.append(sampler.color(x: x, y: y))
    }
    return medianColor(colors)
}

func interiorColor(_ box: VisionBox, sampler: PixelSampler) -> RGB {
    var colors: [RGB] = []
    for yi in 0..<7 {
        for xi in 0..<7 {
            let fx = 0.2 + 0.6 * Double(xi) / 6.0
            let fy = 0.2 + 0.6 * Double(yi) / 6.0
            colors.append(sampler.color(
                x: Int(box.x + box.width * fx),
                y: Int(box.y + box.height * fy)
            ))
        }
    }
    return medianColor(colors)
}
func localBackgroundColor(_ box: VisionBox, sampler: PixelSampler) -> RGB {
    let left = Int(box.x), right = Int(box.x + box.width)
    let top = Int(box.y), bottom = Int(box.y + box.height)
    let pad = max(2, Int(min(box.width, box.height) * 0.12))
    return medianColor([
        sampler.color(x: left - pad, y: top - pad),
        sampler.color(x: right + pad, y: top - pad),
        sampler.color(x: left - pad, y: bottom + pad),
        sampler.color(x: right + pad, y: bottom + pad),
    ])
}

func textColor(_ box: VisionBox, sampler: PixelSampler) -> RGB {
    let left = Int(box.x), right = Int(box.x + box.width)
    let top = Int(box.y), bottom = Int(box.y + box.height)
    let localBackground = localBackgroundColor(box, sampler: sampler)
    let step = max(1, Int(min(box.width, box.height) / 10.0))
    var best = localBackground
    var bestDistance = 0.0
    var y = top
    while y <= bottom {
        var x = left
        while x <= right {
            let candidate = sampler.color(x: x, y: y)
            let distance = candidate.distance(to: localBackground)
            if distance > bestDistance {
                best = candidate
                bestDistance = distance
            }
            x += step
        }
        y += step
    }
    return best
}

struct TypographyInference {
    let size: Double
    let weight: String
    let baselineY: Double
}

func textInkBounds(_ box: VisionBox, textColor: RGB, sampler: PixelSampler) -> VisionBox? {
    let background = localBackgroundColor(box, sampler: sampler)
    let contrast = textColor.distance(to: background)
    guard contrast >= 18.0 else { return nil }
    let left = max(0, Int(floor(box.x)))
    let right = min(sampler.width - 1, Int(ceil(box.x + box.width)))
    let top = max(0, Int(floor(box.y)))
    let bottom = min(sampler.height - 1, Int(ceil(box.y + box.height)))
    let textThreshold = max(20.0, contrast * 0.58)
    let backgroundThreshold = contrast * 0.24
    var minX = right, maxX = left, minY = bottom, maxY = top
    var found = false
    for y in top...bottom {
        for x in left...right {
            let pixel = sampler.color(x: x, y: y)
            guard pixel.distance(to: textColor) <= textThreshold,
                  pixel.distance(to: background) >= backgroundThreshold else { continue }
            minX = min(minX, x); maxX = max(maxX, x)
            minY = min(minY, y); maxY = max(maxY, y)
            found = true
        }
    }
    guard found else { return nil }
    return VisionBox(
        x: Double(minX), y: Double(minY),
        width: Double(maxX - minX + 1), height: Double(maxY - minY + 1)
    )
}

func inferTypography(text: String, box: VisionBox, textColor: RGB, sampler: PixelSampler) -> TypographyInference {
    let fallbackSize = min(512.0, max(8.0, box.height * 0.78))
    let fallback = TypographyInference(size: fallbackSize, weight: "600", baselineY: box.y + box.height * 0.82)
    guard let ink = textInkBounds(box, textColor: textColor, sampler: sampler) else { return fallback }
    let candidates: [(NSFont.Weight, String)] = [(.regular, "400"), (.medium, "500"), (.semibold, "600"), (.bold, "700")]
    var best: (score: Double, size: Double, weight: String, bounds: CGRect)?
    for (nativeWeight, cssWeight) in candidates {
        let font = NSFont.systemFont(ofSize: 100.0, weight: nativeWeight)
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font]))
        let glyphBounds = CTLineGetBoundsWithOptions(line, [.useGlyphPathBounds, .excludeTypographicLeading])
        guard glyphBounds.width > 0.5, glyphBounds.height > 0.5 else { continue }
        let sizeFromWidth = ink.width / Double(glyphBounds.width) * 100.0
        let sizeFromHeight = ink.height / Double(glyphBounds.height) * 100.0
        let score = abs(log(sizeFromWidth / sizeFromHeight))
        let size = min(512.0, max(8.0, sizeFromWidth * 0.65 + sizeFromHeight * 0.35))
        if best == nil || score < best!.score {
            best = (score, size, cssWeight, glyphBounds)
        }
    }
    guard let best else { return fallback }
    let scale = best.size / 100.0
    let minY = Double(best.bounds.minY) * scale
    let maxY = Double(best.bounds.maxY) * scale
    let baselineFromTop = ink.y + maxY
    let baselineFromBottom = ink.y + ink.height + minY
    return TypographyInference(
        size: best.size,
        weight: best.weight,
        baselineY: (baselineFromTop + baselineFromBottom) / 2.0
    )
}

func estimateCornerRadius(_ box: VisionBox, fill: RGB, sampler: PixelSampler) -> Double? {
    let maxRadius = Int(min(box.width, box.height) / 2.0)
    guard maxRadius >= 2 else { return nil }
    let left = Int(box.x)
    let top = Int(box.y)
    var radii: [Int] = []
    for rowOffset in 1...min(4, maxRadius) {
        let y = top + rowOffset
        for dx in 0...maxRadius {
            if sampler.color(x: left + dx, y: y).distance(to: fill) < 14.0 {
                radii.append(dx)
                break
            }
        }
    }
    guard !radii.isEmpty else { return nil }
    let sorted = radii.sorted()
    return Double(sorted[sorted.count / 2])
}
func overlapCoverage(_ a: VisionBox, _ b: VisionBox) -> Double {
    let left = max(a.x, b.x)
    let top = max(a.y, b.y)
    let right = min(a.x + a.width, b.x + b.width)
    let bottom = min(a.y + a.height, b.y + b.height)
    let intersection = max(0.0, right - left) * max(0.0, bottom - top)
    let area = max(1.0, a.width * a.height)
    return intersection / area
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data(("GlyphVision: \(message)\n").utf8))
    exit(1)
}

func glyphBox(_ rect: CGRect, width: Int, height: Int) -> VisionBox {
    VisionBox(
        x: Double(rect.minX) * Double(width),
        y: (1.0 - Double(rect.maxY)) * Double(height),
        width: Double(rect.width) * Double(width),
        height: Double(rect.height) * Double(height)
    )
}

func area(_ box: VisionBox, width: Int, height: Int) -> Double {
    (box.width * box.height) / (Double(width) * Double(height))
}

func run() throws {
    guard CommandLine.arguments.count == 2 else {
        fail("usage: GlyphVision <image-path>")
    }
    let source = URL(fileURLWithPath: CommandLine.arguments[1]).standardizedFileURL
    guard let imageSource = CGImageSourceCreateWithURL(source as CFURL, nil),
          let image = CGImageSourceCreateImageAtIndex(imageSource, 0, nil),
          let sampler = PixelSampler(image: image) else {
        fail("could not decode image at \(source.path)")
    }
    let width = image.width
    let height = image.height
    guard width > 0, height > 0 else { fail("image has invalid dimensions") }
    let canvasBackground = backgroundColor(sampler)

    let textRequest = VNRecognizeTextRequest()
    textRequest.recognitionLevel = .accurate
    textRequest.usesLanguageCorrection = true
    textRequest.minimumTextHeight = 0.012

    let rectangleRequest = VNDetectRectanglesRequest()
    rectangleRequest.maximumObservations = 48
    rectangleRequest.minimumConfidence = 0.5
    rectangleRequest.minimumAspectRatio = 0.05
    rectangleRequest.maximumAspectRatio = 20.0
    rectangleRequest.minimumSize = 0.025
    rectangleRequest.quadratureTolerance = 20.0

    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    try handler.perform([textRequest, rectangleRequest])

    var observations: [VisionObservation] = []
    var textBoxes: [VisionBox] = []
    for observation in textRequest.results ?? [] {
        guard let candidate = observation.topCandidates(1).first else { continue }
        let text = candidate.string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { continue }
        let box = glyphBox(observation.boundingBox, width: width, height: height)
        let color = textColor(box, sampler: sampler)
        let typography = inferTypography(text: text, box: box, textColor: color, sampler: sampler)
        textBoxes.append(box)
        observations.append(VisionObservation(
            kind: "text",
            text: text,
            bbox: box,
            confidence: Double(candidate.confidence),
            color: color.hex,
            corner_radius: nil,
            font_size: typography.size,
            font_weight: typography.weight,
            baseline_y: typography.baselineY
        ))
    }
    for observation in rectangleRequest.results ?? [] {
        let box = glyphBox(observation.boundingBox, width: width, height: height)
        let normalizedArea = area(box, width: width, height: height)
        guard normalizedArea >= 0.001, normalizedArea <= 0.96 else { continue }
        if textBoxes.contains(where: { overlapCoverage(box, $0) >= 0.55 }) {
            continue
        }
        let fill = interiorColor(box, sampler: sampler)
        observations.append(VisionObservation(
            kind: "rectangle",
            text: nil,
            bbox: box,
            confidence: Double(observation.confidence),
            color: fill.hex,
            corner_radius: estimateCornerRadius(box, fill: fill, sampler: sampler),
            font_size: nil,
            font_weight: nil,
            baseline_y: nil
        ))
    }

    let compactLimit = Double(min(width, height)) * 0.18
    for candidate in compactSolidRegions(sampler, background: canvasBackground) {
        let box = candidate.bbox
        if textBoxes.contains(where: { overlapCoverage(box, $0) >= 0.35 }) {
            continue
        }
        observations.removeAll { existing in
            guard existing.kind == "rectangle" else { return false }
            let overlap = max(
                overlapCoverage(box, existing.bbox),
                overlapCoverage(existing.bbox, box)
            )
            return overlap >= 0.40
                && max(existing.bbox.width, existing.bbox.height) <= compactLimit * 1.5
        }
        let duplicate = observations.contains { existing in
            guard existing.kind == "ellipse" else { return false }
            return max(
                overlapCoverage(box, existing.bbox),
                overlapCoverage(existing.bbox, box)
            ) >= 0.65
        }
        if duplicate { continue }
        observations.append(VisionObservation(
            kind: "ellipse",
            text: nil,
            bbox: box,
            confidence: min(0.98, 0.70 + candidate.occupancy * 0.30),
            color: candidate.color.hex,
            corner_radius: min(box.width, box.height) / 2.0,
            font_size: nil,
            font_weight: nil,
            baseline_y: nil
        ))
    }

    observations.sort { lhs, rhs in
        let yDelta = lhs.bbox.y - rhs.bbox.y
        if abs(yDelta) > 0.5 { return yDelta < 0 }
        let xDelta = lhs.bbox.x - rhs.bbox.x
        if abs(xDelta) > 0.5 { return xDelta < 0 }
        if lhs.kind != rhs.kind { return lhs.kind < rhs.kind }
        return (lhs.text ?? "") < (rhs.text ?? "")
    }

    let manifest = VisionManifest(
        schema_version: 1,
        source: source.path,
        width: width,
        height: height,
        background: canvasBackground.hex,
        observations: observations
    )
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    let data = try encoder.encode(manifest)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
}

do {
    try run()
} catch {
    fail(error.localizedDescription)
}
