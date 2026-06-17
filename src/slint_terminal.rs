slint::slint! {
    export struct TerminalCellItem {
        text: string,
        x: length,
        y: length,
        width: length,
        height: length,
        foreground: color,
        background: color,
        bold: bool,
        italic: bool,
    }

    export struct TerminalDecorationItem {
        x: length,
        y: length,
        width: length,
        height: length,
        color: color,
    }

    export component TerminalWindow inherits Window {
        in property <[TerminalCellItem]> cells;
        in property <[TerminalDecorationItem]> decorations;
        in property <string> window-title: "Timon Slint Terminal";
        in property <color> terminal-background: #0f1419;
        in property <string> terminal-font-family: "monospace";
        in property <length> terminal-font-size: 13px;
        out property <length> terminal-native-cell-width: font-measure.preferred-width / 10;
        out property <length> terminal-native-cell-height: font-measure.font-metrics.ascent - font-measure.font-metrics.descent;
        in property <bool> cursor-overlay-visible: false;
        in property <length> cursor-overlay-x: 0px;
        in property <length> cursor-overlay-y: 0px;
        in property <length> cursor-overlay-width: 0px;
        in property <length> cursor-overlay-height: 0px;
        in property <color> cursor-overlay-color: #ffffff;
        callback input(string, bool, bool, bool, bool);
        callback pointer-down(float, float);
        callback pointer-moved(float, float);
        callback pointer-up(float, float);
        callback scroll(float, float, float);
        callback focus-changed(bool);
        callback session-event-ready();

        title: root.window-title;
        background: root.terminal-background;

        focus := FocusScope {
            width: 100%;
            height: 100%;
            focus-on-click: true;
            focus-on-tab-navigation: false;

            capture-key-pressed(event) => {
                if (event.text != "") {
                    root.input(
                        event.text,
                        event.modifiers.alt,
                        event.modifiers.control,
                        event.modifiers.shift,
                        event.modifiers.meta,
                    );
                    return accept;
                }

                return reject;
            }

            focus-gained(_) => {
                root.focus-changed(true);
            }

            focus-lost(_) => {
                root.focus-changed(false);
            }

            Rectangle {
                width: 100%;
                height: 100%;
                background: root.terminal-background;

                font-measure := Text {
                    visible: false;
                    text: "MMMMMMMMMM";
                    font-family: root.terminal-font-family;
                    font-size: root.terminal-font-size;
                    font-weight: 400;
                }

                for cell in root.cells: Rectangle {
                    x: cell.x;
                    y: cell.y;
                    width: cell.width;
                    height: cell.height;
                    background: cell.background;

                    Text {
                        x: 0px;
                        y: 0px;
                        width: parent.width;
                        height: parent.height;
                        text: cell.text;
                        color: cell.foreground;
                        font-family: root.terminal-font-family;
                        font-size: root.terminal-font-size;
                        font-weight: cell.bold ? 700 : 400;
                        font-italic: cell.italic;
                        vertical-alignment: center;
                    }
                }

                for decoration in root.decorations: Rectangle {
                    x: decoration.x;
                    y: decoration.y;
                    width: decoration.width;
                    height: decoration.height;
                    background: decoration.color;
                }

                Rectangle {
                    visible: root.cursor-overlay-visible;
                    x: root.cursor-overlay-x;
                    y: root.cursor-overlay-y;
                    width: root.cursor-overlay-width;
                    height: root.cursor-overlay-height;
                    background: root.cursor-overlay-color;
                }
            }

            touch := TouchArea {
                width: 100%;
                height: 100%;

                pointer-event(event) => {
                    if (event.button != PointerEventButton.left) {
                        return;
                    }

                    if (event.kind == PointerEventKind.down) {
                        root.pointer-down(self.mouse-x / 1px, self.mouse-y / 1px);
                    } else if (event.kind == PointerEventKind.move) {
                        root.pointer-moved(self.mouse-x / 1px, self.mouse-y / 1px);
                    } else if (event.kind == PointerEventKind.up || event.kind == PointerEventKind.cancel) {
                        root.pointer-up(self.mouse-x / 1px, self.mouse-y / 1px);
                    }
                }

                scroll-event(event) => {
                    root.scroll(event.delta-y / 1px, self.mouse-x / 1px, self.mouse-y / 1px);
                    return accept;
                }
            }
        }
    }
}
