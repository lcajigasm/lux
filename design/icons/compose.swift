import AppKit

let args = CommandLine.arguments
guard args.count == 4 else {
    fputs("usage: compose.swift INPUT light|dark OUTPUT\n", stderr)
    exit(2)
}
let input = URL(fileURLWithPath: args[1])
let output = URL(fileURLWithPath: args[3])
guard let icon = NSImage(contentsOf: input) else {
    fputs("cannot read \(input.path)\n", stderr)
    exit(1)
}
let side: CGFloat = 1024
let canvas = NSImage(size: NSSize(width: side, height: side))
canvas.lockFocus()
let color: NSColor = args[2] == "dark"
    ? NSColor(srgbRed: 28.0 / 255.0, green: 28.0 / 255.0, blue: 30.0 / 255.0, alpha: 1)
    : NSColor(srgbRed: 245.0 / 255.0, green: 245.0 / 255.0, blue: 247.0 / 255.0, alpha: 1)
color.setFill()
NSRect(x: 0, y: 0, width: side, height: side).fill()
icon.draw(
    in: NSRect(x: 0, y: 0, width: side, height: side),
    from: NSRect(origin: .zero, size: icon.size),
    operation: .sourceOver,
    fraction: 1
)
canvas.unlockFocus()
guard
    let tiff = canvas.tiffRepresentation,
    let rep = NSBitmapImageRep(data: tiff),
    let png = rep.representation(using: .png, properties: [:])
else {
    fputs("cannot encode png\n", stderr)
    exit(1)
}
try png.write(to: output)
