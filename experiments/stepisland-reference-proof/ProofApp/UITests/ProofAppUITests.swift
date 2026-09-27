import XCTest

final class ProofAppUITests: XCTestCase {
    func testGlyphGeneratedButtonExecutesNativeAction() {
        let app = XCUIApplication()
        app.launch()

        let button = app.buttons["onboarding_primary_action"]
        XCTAssertTrue(button.waitForExistence(timeout: 8))
        XCTAssertTrue(button.isHittable)
        button.tap()

        let confirmation = app.staticTexts["Glyph action executed"]
        XCTAssertTrue(confirmation.waitForExistence(timeout: 5))
        XCTAssertFalse(button.exists)
    }
}
