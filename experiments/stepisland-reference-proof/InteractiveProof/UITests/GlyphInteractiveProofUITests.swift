import XCTest

final class GlyphInteractiveProofUITests: XCTestCase {
    func testReferenceScreenIsActuallyInteractive() throws {
        let app = XCUIApplication()
        app.launch()

        let getStarted = app.buttons["glyph-proof-get-started"]
        XCTAssertTrue(getStarted.waitForExistence(timeout: 6))
        getStarted.tap()

        let secondPage = app.staticTexts["glyph-proof-second-page"]
        XCTAssertTrue(secondPage.waitForExistence(timeout: 4))

        let back = app.buttons["glyph-proof-back"]
        XCTAssertTrue(back.waitForExistence(timeout: 4))
        back.tap()
        XCTAssertTrue(getStarted.waitForExistence(timeout: 4))
    }
}
