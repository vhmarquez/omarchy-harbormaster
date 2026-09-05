import QtQuick
import QtTest

// Native toolchain sentinel only, not a manager UI or domain implementation.
Item {
    id: root

    height: 48
    width: 64

    TestCase {
        function init() {
            failOnWarning(/.*/);
        }
        function test_native_fixture_dimensions() {
            compare(root.width, 64);
            compare(root.height, 48);
        }

        name: "Toolchain"
        when: windowShown
    }
}
