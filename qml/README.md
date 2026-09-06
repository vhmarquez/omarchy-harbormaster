# Native QML foundation

`tests/tst_toolchain.qml` is a Qt Quick Test import/engine sentinel, not a
manager screen, application state model or evidence of UX/accessibility parity.
It requires real Qt 6.11.2 `qmllint`, `qmlformat` and `qmltestrunner`, run offline
with the software/offscreen backend through `python3 scripts/verify.py`.
Missing tools/imports, warnings, empty or skipped suites fail required gates.

Production QML and Omarchy/Quickshell integration are later work. Preserve the
approved option 02 and supplemental design under `docs/design`; this fixture
does not replace those designs or load/restart the live desktop shell.
