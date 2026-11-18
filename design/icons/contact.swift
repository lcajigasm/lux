import AppKit

let args = CommandLine.arguments
guard args.count == 20 else {
    fputs("usage: contact.swift OUT 6 of the 1024 pngs, 6 of the 128 pngs, 6 of the 32 pngs\n", stderr)
    exit(2)
}

func load(_ path: String) -> NSImage {
    guard let image = NSImage(contentsOf: URL(fileURLWithPath: path)) else {
        fputs("cannot read \(path)\n", stderr)
        exit(1)
    }
    return image
}

let names = ["slot", "meter", "corner"]
let large = (0..<6).map { load(args[2 + $0]) }
let mid = (0..<6).map { load(args[8 + $0]) }
let tiny = (0..<6).map { load(args[14 + $0]) }

let margin: CGFloat = 40
let gap: CGFloat = 24
let col: CGFloat = 280
let width = margin * 2 + col * 3 + gap * 2
let height: CGFloat = 1040
let canvas = NSImage(size: NSSize(width: width, height: height))

func draw(_ image: NSImage, _ rect: NSRect, nearest: Bool) {
    NSGraphicsContext.current?.imageInterpolation = nearest ? .none : .high
    image.draw(
        in: rect,
        from: NSRect(origin: .zero, size: image.size),
        operation: .sourceOver,
        fraction: 1
    )
}

func caption(_ text: String, _ x: CGFloat, _ baseline: CGFloat, _ size: CGFloat) {
    let font = NSFont.systemFont(ofSize: size, weight: .medium)
    let attrs: [NSAttributedString.Key: Any] = [
        .font: font,
        .foregroundColor: NSColor(srgbRed: 0.12, green: 0.12, blue: 0.13, alpha: 1),
    ]
    NSString(string: text).draw(at: NSPoint(x: x, y: baseline), withAttributes: attrs)
}

canvas.lockFocus()
NSColor(srgbRed: 0.93, green: 0.93, blue: 0.94, alpha: 1).setFill()
NSRect(x: 0, y: 0, width: width, height: height).fill()
caption("SVG via librsvg. Each column: 1024 on light, 1024 on dark, then the 128 files, then the 32 files scaled nearest-neighbor.", margin, height - 58, 13)
for index in 0..<3 {
    let x = margin + CGFloat(index) * (col + gap)
    caption(names[index], x, height - 88, 18)
    var y = height - 104 - col
    draw(large[index * 2], NSRect(x: x, y: y, width: col, height: col), nearest: false)
    y -= gap + col
    draw(large[index * 2 + 1], NSRect(x: x, y: y, width: col, height: col), nearest: false)
    y -= 16 + 128
    draw(mid[index * 2], NSRect(x: x, y: y, width: 128, height: 128), nearest: false)
    draw(mid[index * 2 + 1], NSRect(x: x + 148, y: y, width: 128, height: 128), nearest: false)
    y -= 16 + 128
    draw(tiny[index * 2], NSRect(x: x, y: y, width: 128, height: 128), nearest: true)
    draw(tiny[index * 2 + 1], NSRect(x: x + 148, y: y, width: 128, height: 128), nearest: true)
}
canvas.unlockFocus()

guard
    let tiff = canvas.tiffRepresentation,
    let rep = NSBitmapImageRep(data: tiff),
    let png = rep.representation(using: .png, properties: [:])
else {
    fputs("cannot encode contact sheet\n", stderr)
    exit(1)
}
try png.write(to: URL(fileURLWithPath: args[1]))
