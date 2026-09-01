import UIKit
import MetalKit

/// Swift host for Phase 12: owns `MTKView` / `CAMetalLayer` and talks to
/// the Rust staticlib. Not a winit embed (ADR 0004).
final class LuxViewController: UIViewController, UIKeyInput, UITextInputTraits {
    let metalView = MTKView()
    var documentURL: URL?

    override var canBecomeFirstResponder: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.addSubview(metalView)
        metalView.frame = view.bounds
        metalView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        becomeFirstResponder()
    }

    override var keyCommands: [UIKeyCommand]? {
        [UIKeyCommand(input: "p", modifierFlags: .command, action: #selector(gotoAnything))]
    }

    @objc func gotoAnything() {}

    func insertText(_ text: String) {
        lux_ios_insert_text(text)
    }

    func deleteBackward() {
        lux_ios_delete_backward()
    }

    var hasText: Bool { true }

    func pickDocument() {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.item])
        present(picker, animated: true)
    }

    func startAccessing(_ url: URL) {
        _ = url.startAccessingSecurityScopedResource()
        documentURL = url
    }
}

@_silgen_name("lux_ios_insert_text")
func lux_ios_insert_text(_ text: String)

@_silgen_name("lux_ios_delete_backward")
func lux_ios_delete_backward()
