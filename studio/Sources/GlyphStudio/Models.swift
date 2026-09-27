import Foundation

struct DisplayList: Decodable {
    let schemaVersion: Int
    let sceneId: String
    let revision: Int
    let canvas: DisplayCanvas
    let items: [DisplayItem]
    let objectMap: [String: ObjectRecord]

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case sceneId = "scene_id"
        case revision, canvas, items
        case objectMap = "object_map"
    }
}

struct DisplayCanvas: Decodable {
    let width: Double
    let height: Double
    let background: String?
}

struct DisplayItem: Decodable, Identifiable {
    let id: String
    let name: String
    let roles: [String]
    let bounds: [Double]?
    let primitive: Primitive
    let style: DisplayStyle
}

struct ObjectRecord: Decodable {
    let index: Int
    let name: String
    let kind: String
    let roles: [String]
    let bounds: [Double]?
    let provenance: Provenance
}

struct Provenance: Decodable {
    let source: String
    let line: Int
    let authored: Bool
}

struct DisplayStyle: Decodable {
    let fill: String?
    let stroke: String?
    let strokeWidth: Double
    let opacity: Double
    let fillOpacity: Double
    let strokeOpacity: Double
    let strokeLinecap: String?
    let strokeLinejoin: String?

    enum CodingKeys: String, CodingKey {
        case fill, stroke, opacity
        case strokeWidth = "stroke_width"
        case fillOpacity = "fill_opacity"
        case strokeOpacity = "stroke_opacity"
        case strokeLinecap = "stroke_linecap"
        case strokeLinejoin = "stroke_linejoin"
    }
}

struct Primitive: Decodable {
    let kind: String
    let x: Double?
    let y: Double?
    let width: Double?
    let height: Double?
    let cx: Double?
    let cy: Double?
    let rx: Double?
    let ry: Double?
    let d: String?
    let lines: [String]?
    let lineHeight: Double?
    let font: String?
    let size: Double?
    let weight: String?
    let align: String?

    enum CodingKeys: String, CodingKey {
        case kind, x, y, width, height, cx, cy, rx, ry, d, lines, font, size, weight, align
        case lineHeight = "line_height"
    }
}


struct VisualDiffReport: Decodable {
    let sceneId: String
    let beforeRevision: Int
    let afterRevision: Int
    let canvasChanged: Bool
    let changedObjects: [VisualObjectDiff]

    enum CodingKeys: String, CodingKey {
        case sceneId = "scene_id"
        case beforeRevision = "before_revision"
        case afterRevision = "after_revision"
        case canvasChanged = "canvas_changed"
        case changedObjects = "changed_objects"
    }
}

struct VisualObjectDiff: Decodable, Identifiable {
    let id: String
    let status: String
    let boundsBefore: [Double]?
    let boundsAfter: [Double]?
    let moved: Bool
    let resized: Bool
    let geometryChanged: Bool
    let styleChanged: Bool
    let contentChanged: Bool
    let semanticChanged: Bool
    let changedFields: [String]

    enum CodingKeys: String, CodingKey {
        case id, status, moved, resized
        case boundsBefore = "bounds_before"
        case boundsAfter = "bounds_after"
        case geometryChanged = "geometry_changed"
        case styleChanged = "style_changed"
        case contentChanged = "content_changed"
        case semanticChanged = "semantic_changed"
        case changedFields = "changed_fields"
    }
}

struct DirectManipulationPlan: Decodable {
    let nodeId: String
    let dx: Double
    let dy: Double
    let allowed: Bool
    let constraintEdits: [ConstraintOffsetEdit]
    let instanceParameterEdits: [ComponentParameterEdit]
    let blocked: [ManipulationBlock]
    let note: String

    enum CodingKeys: String, CodingKey {
        case nodeId = "node_id"
        case constraintEdits = "constraint_edits"
        case instanceParameterEdits = "instance_parameter_edits"
        case dx, dy, allowed, blocked, note
    }
}

struct ConstraintOffsetEdit: Decodable, Identifiable {
    let constraintId: String
    let sourceOrder: Int
    let axis: String
    let previousOffset: Double
    let nextOffset: Double
    let unit: String?

    var id: String { constraintId }

    enum CodingKeys: String, CodingKey {
        case constraintId = "constraint_id"
        case sourceOrder = "source_order"
        case previousOffset = "previous_offset"
        case nextOffset = "next_offset"
        case axis, unit
    }
}

struct ManipulationBlock: Decodable, Identifiable {
    let axis: String
    let constraintIds: [String]
    let properties: [String]
    let reason: String

    var id: String { axis }

    enum CodingKeys: String, CodingKey {
        case axis, properties, reason
        case constraintIds = "constraint_ids"
    }
}

struct ComponentParameterEdit: Decodable, Identifiable {
    let instanceId: String
    let parameter: String
    let targetProperty: String
    let axis: String

    var id: String { "\(instanceId).\(parameter).\(axis)" }

    enum CodingKeys: String, CodingKey {
        case instanceId = "instance_id"
        case targetProperty = "target_property"
        case parameter, axis
    }
}

struct DirectResizePlan: Decodable {
    let nodeId: String
    let dw: Double
    let dh: Double
    let allowed: Bool
    let constraintEdits: [ConstraintOffsetEdit]
    let instanceParameterEdits: [ComponentParameterEdit]
    let blocked: [ManipulationBlock]
    let note: String

    enum CodingKeys: String, CodingKey {
        case nodeId = "node_id"
        case constraintEdits = "constraint_edits"
        case instanceParameterEdits = "instance_parameter_edits"
        case dw, dh, allowed, blocked, note
    }
}

struct ConstraintPropertyReference: Decodable {
    let node: String
    let property: String

    var display: String { "\(node).\(property)" }
}

struct ConstraintQuantity: Decodable {
    let value: Double
    let unit: String
}

struct ConstraintRHS: Decodable {
    let kind: String
    let property: ConstraintPropertyReference?
    let number: Double?
    let quantity: ConstraintQuantity?

    private enum CodingKeys: String, CodingKey { case kind, value }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        kind = try container.decode(String.self, forKey: .kind)
        switch kind {
        case "Property":
            property = try container.decode(ConstraintPropertyReference.self, forKey: .value)
            number = nil
            quantity = nil
        case "Number":
            property = nil
            number = try container.decode(Double.self, forKey: .value)
            quantity = nil
        case "Quantity":
            property = nil
            number = nil
            quantity = try container.decode(ConstraintQuantity.self, forKey: .value)
        default:
            property = nil
            number = nil
            quantity = nil
        }
    }

    var display: String {
        if let property { return property.display }
        if let number { return Self.format(number) }
        if let quantity { return "\(Self.format(quantity.value))\(quantity.unit)" }
        return "?"
    }

    private static func format(_ value: Double) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%g", value)
    }
}

struct ConstraintRecord: Decodable, Identifiable {
    let id: String
    let lhs: ConstraintPropertyReference
    let relation: String
    let rhs: ConstraintRHS
    let offset: Double
    let offsetUnit: String?
    let strength: String
    let sourceOrder: Int

    enum CodingKeys: String, CodingKey {
        case id, lhs, relation, rhs, offset, strength
        case offsetUnit = "offset_unit"
        case sourceOrder = "source_order"
    }

    func involves(_ nodeID: String) -> Bool {
        lhs.node == nodeID || rhs.property?.node == nodeID
    }

    var relationSymbol: String {
        switch relation {
        case "ge": return ">="
        case "le": return "<="
        default: return "=="
        }
    }

    var displayOffset: String {
        guard abs(offset) > Double.ulpOfOne else { return "" }
        let sign = offset < 0 ? "−" : "+"
        let magnitude = abs(offset)
        let value = magnitude.rounded() == magnitude
            ? String(Int(magnitude))
            : String(format: "%g", magnitude)
        return " \(sign) \(value)\(offsetUnit ?? "")"
    }

    var displayExpression: String {
        "\(lhs.display) \(relationSymbol) \(rhs.display)\(displayOffset)"
    }
}
